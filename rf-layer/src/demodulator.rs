//! Demodulator state management with bi-temporal provenance and snapshotting
//!
//! This implements the demodulator state that's shared between primary and shadow SDRs
//! for lossless failover (Problem 1). Snapshots are taken periodically for recovery.

use batching::DemodulatorBatcher;
use bitemporal::timestamp::{BiTemporal, EventTime, ReceptionTime};
use chrono::{DateTime, Utc};
use ground_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::sdr::{Sample, SampleId};

/// Costas/Gardner loop constants
const COSTAS_ALPHA: f64 = 0.02; // Proportional gain (phase correction per sample)
const COSTAS_BETA: f64 = 4e-4; // Integral gain (frequency correction per sample)
const GARDNER_GAIN: f64 = 0.02; // Symbol-timing correction gain
const DEFAULT_SAMPLES_PER_SYMBOL: f64 = 8.0; // 8 samples per symbol at default baud rate

/// Demodulator state that can be snapshotted and restored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemodState {
    /// Current NCO phase (cycles, 0.0 to 1.0) — kept for snapshot compatibility
    pub nco_phase: f64,
    /// Current frequency offset estimate (Hz)
    pub frequency_offset: i64,
    /// Symbol synchronization state
    pub symbol_sync: SymbolSyncState,
    /// Carrier lock status
    pub carrier_locked: bool,
    /// Current bit buffer (collected bits, 8 at a time assemble a byte)
    pub bit_buffer: Vec<u8>,
    /// Last committed sample ID (for bi-temporal provenance)
    pub last_sample_id: SampleId,

    // ── Costas loop (carrier recovery) ──────────────────────────────────────
    /// Carrier phase estimate (radians)
    pub carrier_phase_rad: f64,
    /// Carrier frequency correction (radians/sample)
    pub carrier_freq_rad_per_sample: f64,

    // ── Gardner timing recovery ───────────────────────────────────────────────
    /// Nominal samples per symbol (sample_rate / baud_rate)
    pub samples_per_symbol: f64,
    /// Fractional position within the current symbol (0.0 → 1.0)
    pub symbol_phase: f64,
    /// Baseband I sample at the previous symbol decision point
    pub prev_decision_i: f32,
    /// Baseband I sample at the midpoint between the last two decision points
    pub midpoint_i: f32,
}

impl DemodState {
    pub fn new() -> Self {
        Self {
            nco_phase: 0.0,
            frequency_offset: 0,
            symbol_sync: SymbolSyncState::new(),
            carrier_locked: false,
            bit_buffer: Vec::new(),
            last_sample_id: SampleId::new(0),
            carrier_phase_rad: 0.0,
            carrier_freq_rad_per_sample: 0.0,
            samples_per_symbol: DEFAULT_SAMPLES_PER_SYMBOL,
            symbol_phase: 0.0,
            prev_decision_i: 0.0,
            midpoint_i: 0.0,
        }
    }

    /// Create a demodulator state pre-configured for a known baud rate.
    pub fn with_baud_rate(sample_rate_hz: u32, baud_rate_hz: u32) -> Self {
        let mut s = Self::new();
        s.samples_per_symbol = sample_rate_hz as f64 / baud_rate_hz as f64;
        s
    }

    /// Process one complex IQ sample through the BPSK demodulator pipeline.
    ///
    /// Pipeline:
    /// 1. Costas loop carrier recovery (second-order PLL)
    /// 2. Gardner symbol timing recovery
    /// 3. Hard-decision bit extraction at symbol boundaries
    /// 4. Byte assembly from 8 consecutive bits
    ///
    /// Returns `Ok(Some(byte))` when a complete byte is available, `Ok(None)` otherwise.
    pub fn process_sample(&mut self, sample: &Sample) -> Result<Option<u8>> {
        self.last_sample_id = sample.id;

        // ── 1. Costas loop carrier recovery ─────────────────────────────────
        // Rotate input sample by the current carrier phase estimate to
        // bring the signal to baseband.
        let cos_phi = self.carrier_phase_rad.cos();
        let sin_phi = self.carrier_phase_rad.sin();
        let i_in = sample.i as f64;
        let q_in = sample.q as f64;
        let i_b = i_in * cos_phi + q_in * sin_phi;
        let q_b = -i_in * sin_phi + q_in * cos_phi;

        // BPSK Costas phase error discriminator:
        // e = sgn(I) * Q  (zero when phase is perfectly recovered)
        let carrier_error = if i_b >= 0.0 { q_b } else { -q_b };

        // Second-order PLL loop filter
        self.carrier_freq_rad_per_sample += COSTAS_BETA * carrier_error;
        self.carrier_phase_rad = (self.carrier_phase_rad
            + self.carrier_freq_rad_per_sample
            + COSTAS_ALPHA * carrier_error)
            .rem_euclid(2.0 * std::f64::consts::PI);

        // Update legacy fields for snapshot compatibility
        self.nco_phase = self.carrier_phase_rad / (2.0 * std::f64::consts::PI);
        self.frequency_offset =
            (self.carrier_freq_rad_per_sample * 1e6 / (2.0 * std::f64::consts::PI)) as i64;

        // Carrier lock: declared when the Costas error has settled
        self.carrier_locked = carrier_error.abs() < 0.1;

        // ── 2. Gardner symbol timing recovery ────────────────────────────────
        let step = 1.0 / self.samples_per_symbol;
        let prev_phase = self.symbol_phase;
        self.symbol_phase += step;

        // Sample at the midpoint between decision instants
        if prev_phase < 0.5 && self.symbol_phase >= 0.5 {
            self.midpoint_i = i_b as f32;
        }

        // At symbol boundary: make a decision and update timing
        if self.symbol_phase >= 1.0 {
            self.symbol_phase -= 1.0;

            // Gardner timing error: e = I_{midpoint} * (I_now - I_prev)
            let timing_error = self.midpoint_i as f64 * (i_b - self.prev_decision_i as f64);

            // Adjust symbol clock phase (fractional correction)
            let correction = GARDNER_GAIN * timing_error;
            self.symbol_phase = (self.symbol_phase - correction).clamp(-0.5, 0.5).abs();

            // Update sync state for monitoring/logging
            self.symbol_sync.timing_offset = timing_error;
            self.symbol_sync.clock_phase = self.symbol_phase;
            self.symbol_sync.error_accumulator += timing_error.abs();

            // ── 3. Hard decision ─────────────────────────────────────────────
            let bit: u8 = if i_b >= 0.0 { 1 } else { 0 };
            self.bit_buffer.push(bit);
            self.prev_decision_i = i_b as f32;

            // ── 4. Byte assembly ─────────────────────────────────────────────
            if self.bit_buffer.len() >= 8 {
                let byte = self
                    .bit_buffer
                    .drain(..8)
                    .fold(0u8, |acc, b| (acc << 1) | b);
                return Ok(Some(byte));
            }
        }

        Ok(None)
    }
}

impl Default for DemodState {
    fn default() -> Self {
        Self::new()
    }
}

/// Symbol synchronization state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSyncState {
    /// Current symbol timing offset
    pub timing_offset: f64,
    /// Symbol clock phase
    pub clock_phase: f64,
    /// Error accumulator for timing recovery
    pub error_accumulator: f64,
}

impl SymbolSyncState {
    pub fn new() -> Self {
        Self {
            timing_offset: 0.0,
            clock_phase: 0.0,
            error_accumulator: 0.0,
        }
    }
}

impl Default for SymbolSyncState {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of demodulator state for failover recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemodulatorSnapshot {
    /// Sample ID at which this snapshot was taken
    pub sample_id: SampleId,
    /// Complete demodulator state
    pub state: DemodState,
    /// When this snapshot was captured (reception time)
    pub captured_at_reception: DateTime<Utc>,
    /// When this snapshot was captured (event time, from satellite)
    pub captured_at_event: DateTime<Utc>,
    /// Checksum for integrity verification
    pub checksum: u64,
}

impl DemodulatorSnapshot {
    /// Create a snapshot from current demodulator state
    pub fn capture(state: &DemodState, sample: &Sample) -> Self {
        let checksum = Self::compute_checksum(state);

        Self {
            sample_id: sample.id,
            state: state.clone(),
            captured_at_reception: sample.reception_time,
            captured_at_event: sample.event_time,
            checksum,
        }
    }

    /// Verify the integrity of a snapshot
    pub fn verify(&self) -> bool {
        Self::compute_checksum(&self.state) == self.checksum
    }

    /// Compute checksum of demodulator state
    fn compute_checksum(state: &DemodState) -> u64 {
        let mut hash: u64 = 0;
        hash = hash.wrapping_add(state.nco_phase.to_bits());
        hash = hash.wrapping_add(state.frequency_offset as u64);
        hash = hash.wrapping_add(state.carrier_locked as u64);
        hash = hash.wrapping_add(state.last_sample_id.as_u64());
        hash
    }

    /// Restore demodulator state from this snapshot
    pub fn restore(&self) -> DemodState {
        if !self.verify() {
            tracing::warn!("Restoring from corrupted snapshot");
        }
        self.state.clone()
    }
}

/// Snapshot manager for periodic state capture
pub struct SnapshotManager {
    snapshots: VecDeque<DemodulatorSnapshot>,
    max_snapshots: usize,
    snapshot_interval_samples: u64,
    last_snapshot_sample: SampleId,
}

impl SnapshotManager {
    pub fn new(max_snapshots: usize, interval_samples: u64) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(max_snapshots),
            max_snapshots,
            snapshot_interval_samples: interval_samples,
            last_snapshot_sample: SampleId::new(0),
        }
    }

    pub fn should_snapshot(&self, sample_id: SampleId) -> bool {
        sample_id.as_u64() - self.last_snapshot_sample.as_u64() >= self.snapshot_interval_samples
    }

    pub fn capture_if_needed(
        &mut self,
        state: &DemodState,
        sample: &Sample,
    ) -> Option<DemodulatorSnapshot> {
        if self.should_snapshot(sample.id) {
            let snapshot = DemodulatorSnapshot::capture(state, sample);
            self.add_snapshot(snapshot.clone());
            self.last_snapshot_sample = sample.id;
            Some(snapshot)
        } else {
            None
        }
    }

    fn add_snapshot(&mut self, snapshot: DemodulatorSnapshot) {
        self.snapshots.push_back(snapshot);
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots.pop_front();
        }
    }

    pub fn latest(&self) -> Option<&DemodulatorSnapshot> {
        self.snapshots.back()
    }

    pub fn find_closest(&self, sample_id: SampleId) -> Option<&DemodulatorSnapshot> {
        self.snapshots
            .iter()
            .min_by_key(|s| (s.sample_id.as_u64() as i64 - sample_id.as_u64() as i64).abs())
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

/// Demodulator output with batching for efficient delivery
pub struct DemodulatorOutput {
    /// Batched symbols for output
    batcher: DemodulatorBatcher,
}

impl DemodulatorOutput {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batcher: DemodulatorBatcher::new(batch_size),
        }
    }

    /// Add a decoded symbol with bi-temporal timestamps
    pub fn add_symbol(&mut self, symbol: u8, sample: &Sample) {
        let bi_temporal = BiTemporal::new(
            symbol,
            EventTime::new(sample.event_time),
            ReceptionTime::new(sample.reception_time),
        );
        self.batcher.add_symbol(bi_temporal);
    }

    /// Check if batch is ready to flush
    pub fn is_ready(&self) -> bool {
        self.batcher.is_ready()
    }

    /// Flush the current batch
    pub fn flush(&mut self) -> Option<batching::DemodulatorBatch> {
        self.batcher.flush()
    }

    /// Force flush regardless of batch size
    pub fn force_flush(&mut self) -> Option<batching::DemodulatorBatch> {
        self.batcher.force_flush()
    }
}

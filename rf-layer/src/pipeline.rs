//! Pipeline integration: SDR → NCO → Demodulator
//!
//! This implements the missing integration layer that wires together:
//! - SDR sample ingestion
//! - Doppler NCO correction (phase-continuous frequency adjustment)
//! - Demodulator state machine (Costas loop + Gardner timing)
//!
//! This is the real-time hot path for RF signal processing.

use crate::demodulator::DemodState;
use crate::doppler::{DopplerSchedule, NcoController};
use crate::sdr::{Sample, SampleId, SdrHandle};
use batching::DemodulatorBatcher;
use bitemporal::timestamp::{BiTemporal, EventTime, ReceptionTime};
use chrono::Utc;
use ground_core::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Real-time processing pipeline for RF signal reception
pub struct RfPipeline {
    /// SDR hardware handle
    sdr: Arc<SdrHandle>,
    /// Demodulator state (for BPSK demodulation)
    demodulator: Arc<RwLock<DemodState>>,
    /// NCO controller for Doppler correction
    nco: NcoController,
    /// Doppler schedule for this pass
    doppler_schedule: DopplerSchedule,
    /// Batcher for output symbols
    batcher: DemodulatorBatcher,
    /// Whether the pipeline is running
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl RfPipeline {
    /// Create a new RF processing pipeline
    pub fn new(
        sdr: Arc<SdrHandle>,
        doppler_schedule: DopplerSchedule,
        sample_rate_hz: u32,
        baud_rate_hz: u32,
    ) -> Self {
        let demodulator = Arc::new(RwLock::new(DemodState::with_baud_rate(
            sample_rate_hz,
            baud_rate_hz,
        )));
        let nco = NcoController::new(doppler_schedule.base_frequency);
        let batcher = DemodulatorBatcher::new(1000); // Batch 1000 symbols
        let running = Arc::new(std::sync::atomic::AtomicBool::new(false));

        Self {
            sdr,
            demodulator,
            nco,
            doppler_schedule,
            batcher,
            running,
        }
    }

    /// Start the real-time processing pipeline
    pub async fn start(&mut self) -> Result<()> {
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Starting RF pipeline for pass");

        let mut sample_buffer = vec![Sample::default(); 4096]; // 4K sample buffer

        while self.running.load(std::sync::atomic::Ordering::SeqCst) {
            // 1. Read samples from SDR hardware
            let samples_read = self.sdr.read_samples(&mut sample_buffer)?;

            if samples_read == 0 {
                // No samples available, brief sleep
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                continue;
            }

            // Process each sample through the pipeline
            for sample in &sample_buffer[..samples_read] {
                // 2. Apply Doppler NCO correction (phase-continuous frequency adjustment)
                let corrected_sample = self.apply_doppler_correction(sample)?;

                // 3. Feed to demodulator (Costas loop + Gardner timing recovery)
                let mut demod = self.demodulator.write().await;
                let byte = demod.process_sample(&corrected_sample)?;
                drop(demod); // Drop lock before calling flush_batch

                if let Some(byte) = byte {
                    // 4. Output decoded byte with bi-temporal timestamps
                    let bi_temporal = BiTemporal::new(
                        byte,
                        EventTime::new(sample.event_time),
                        ReceptionTime::new(sample.reception_time),
                    );
                    self.batcher.add_symbol(bi_temporal);

                    // Flush batch if full
                    if self.batcher.is_ready() {
                        self.flush_batch().await;
                    }
                }
            }
        }

        // Final flush
        self.flush_batch().await;

        Ok(())
    }

    /// Stop the processing pipeline
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Stopping RF pipeline");
    }

    /// Apply Doppler NCO correction to a sample
    fn apply_doppler_correction(&self, sample: &Sample) -> Result<Sample> {
        // TODO: Get the Doppler offset for this sample ID from the schedule
        // For now, use 0 as placeholder
        let frequency_offset = 0;

        // Apply NCO phase rotation
        let corrected = self.nco.apply_correction(sample, frequency_offset)?;

        Ok(corrected)
    }

    /// Flush the current batch of decoded symbols
    async fn flush_batch(&mut self) {
        if let Some(batch) = self.batcher.flush() {
            // In a real implementation, this would send the batch to the next processing stage
            // (e.g., frame synchronization, de-interleaving, FEC decoding)
            tracing::debug!("Flushed batch of {} symbols", batch.symbols.len());
        }
    }

    /// Get current pipeline statistics
    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            is_running: self.running.load(std::sync::atomic::Ordering::SeqCst),
            batch_size: 0,               // TODO: implement tracking of batch size
            doppler_schedule_entries: 0, // TODO: implement tracking of schedule entries
        }
    }
}

/// Pipeline statistics
#[derive(Debug, Clone)]
pub struct PipelineStats {
    pub is_running: bool,
    pub batch_size: usize,
    pub doppler_schedule_entries: usize,
}

/// Default sample for buffer initialization
impl Default for Sample {
    fn default() -> Self {
        Sample {
            i: 0.0,
            q: 0.0,
            event_time: Utc::now(),
            reception_time: Utc::now(),
            id: SampleId::new(0),
        }
    }
}

/// Extended NCO controller with sample-level correction
impl NcoController {
    /// Apply frequency/phase correction to a single sample
    pub fn apply_correction(&self, sample: &Sample, frequency_offset: i64) -> Result<Sample> {
        use std::f64::consts::PI;

        // Convert frequency offset to phase increment per sample
        // Δφ = 2π * f_offset / sample_rate
        let sample_rate = 2_000_000.0; // 2 MSPS default
        let phase_increment = 2.0 * PI * (frequency_offset as f64) / sample_rate;

        // Update NCO phase
        let current_phase = self.get_phase();
        let new_phase = (current_phase + phase_increment).rem_euclid(2.0 * PI);

        // Apply phase rotation to the sample
        let cos_phi = new_phase.cos();
        let sin_phi = new_phase.sin();

        let i_corrected = sample.i as f64 * cos_phi - sample.q as f64 * sin_phi;
        let q_corrected = sample.i as f64 * sin_phi + sample.q as f64 * cos_phi;

        Ok(Sample {
            i: i_corrected as f32,
            q: q_corrected as f32,
            event_time: sample.event_time,
            reception_time: sample.reception_time,
            id: sample.id,
        })
    }

    /// Get current NCO phase (simplified - in real implementation would track state)
    fn get_phase(&self) -> f64 {
        0.0 // Placeholder - real implementation would track accumulated phase
    }
}

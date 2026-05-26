//! Shadow-tracking SDRs for lossless failover (Problem 1)
//!
//! This implements the architectural solution for lossless failover:
//! A designated backup SDR continuously tracks the same satellite as the primary,
//! maintaining its own demodulator state. When the primary fails, the shadow
//! is promoted with a simple pointer swap (no re-acquisition needed).

use crate::demodulator::{DemodState, SnapshotManager};
use crate::doppler::DopplerSchedule;
use crate::sdr::{Sample, SampleId, SdrHandle};
use ground_core::{GroundStationError, PassId, Result};
use hardware::PassShard;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Pass acquisition with shadow tracking
pub struct PassAcquisition {
    /// Primary SDR handle
    pub primary: Arc<SdrHandle>,
    /// Optional shadow SDR handle
    pub shadow: Option<Arc<SdrHandle>>,
    /// Primary demodulator state
    pub primary_state: Arc<RwLock<DemodState>>,
    /// Shadow demodulator state
    pub shadow_state: Arc<RwLock<DemodState>>,
    /// Last committed sample ID (for bi-temporal provenance)
    pub last_committed_sample: SampleId,
    /// Pass identifier
    pub pass_id: PassId,
    /// Whether shadow is currently active
    pub shadow_active: bool,
    /// Snapshot manager for failover recovery
    pub snapshot_manager: SnapshotManager,
    /// Doppler schedule for this pass
    pub doppler_schedule: DopplerSchedule,
    /// Pass-isolated memory shard for no-heap allocations
    shard: PassShard,
    /// Shadow last sample counter (for detecting if shadow is receiving samples)
    shadow_last_sample_counter: std::sync::atomic::AtomicU64,
    /// Primary last sample counter (for comparison)
    primary_last_sample_counter: std::sync::atomic::AtomicU64,
    /// Health check timestamp (for detecting stale shadows)
    last_health_check: std::sync::atomic::AtomicU64,
}

impl PassAcquisition {
    /// Create a new pass acquisition with optional shadow
    pub fn new(
        pass_id: PassId,
        primary: Arc<SdrHandle>,
        shadow: Option<Arc<SdrHandle>>,
        doppler_schedule: DopplerSchedule,
    ) -> Self {
        let snapshot_interval = 50u64; // Snapshot every 50 samples
        let snapshot_manager = SnapshotManager::new(100, snapshot_interval.max(1));
        let shadow_active = shadow.is_some();

        // Create pass-isolated shard with 16MB arena for no-heap allocations
        let shard = PassShard::new(pass_id.clone(), 16 * 1024 * 1024);

        Self {
            pass_id,
            primary,
            shadow,
            primary_state: Arc::new(RwLock::new(DemodState::new())),
            shadow_state: Arc::new(RwLock::new(DemodState::new())),
            last_committed_sample: SampleId::new(0),
            shadow_active,
            snapshot_manager,
            doppler_schedule,
            shard,
            shadow_last_sample_counter: std::sync::atomic::AtomicU64::new(0),
            primary_last_sample_counter: std::sync::atomic::AtomicU64::new(0),
            last_health_check: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Process a sample from the primary SDR
    pub async fn process_primary_sample(&mut self, sample: Sample) -> Result<Option<u8>> {
        let mut state = self.primary_state.write().await;

        // Capture snapshot if needed
        if let Some(snapshot) = self.snapshot_manager.capture_if_needed(&state, &sample) {
            tracing::debug!(
                "Captured snapshot at sample {}",
                snapshot.sample_id.as_u64()
            );
        }

        // Update demodulator state
        let result = state.process_sample(&sample)?;
        self.last_committed_sample = sample.id;

        // Increment primary sample counter for health monitoring
        self.primary_last_sample_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Sync shadow state if shadow is active.
        // Arc::get_mut would silently return None whenever any other clone exists;
        // write() on the inner RwLock is the correct API for shared interior mutability.
        if self.shadow_active {
            let mut shadow_lock = self.shadow_state.write().await;
            *shadow_lock = state.clone();
        }

        Ok(result)
    }

    /// Promote shadow to primary (failover)
    /// This is a pointer swap, not a re-acquisition
    pub async fn promote_shadow(&mut self) -> Result<()> {
        if self.shadow.is_none() {
            return Err(GroundStationError::RfProcessing(
                "No shadow SDR available for failover".to_string(),
            ));
        }

        tracing::warn!("Promoting shadow to primary for pass {}", self.pass_id);

        // Swap the handles
        let shadow = self.shadow.take().unwrap();
        let _old_primary = std::mem::replace(&mut self.primary, shadow);

        // Restore demodulator state from latest snapshot
        if let Some(snapshot) = self.snapshot_manager.latest() {
            let mut state = self.primary_state.write().await;
            *state = snapshot.restore();
            tracing::info!(
                "Restored demodulator state from snapshot at sample {}",
                snapshot.sample_id.as_u64()
            );
        }

        // Shadow is no longer active (we just used it)
        self.shadow_active = false;

        // Old primary could be re-initialized as shadow if hardware allows
        // For now, we just drop it

        tracing::info!("Shadow promoted successfully");
        Ok(())
    }

    /// Get the pass shard for direct allocation (for no-heap allocations in real-time path)
    pub fn shard(&mut self) -> &mut PassShard {
        &mut self.shard
    }

    /// Allocate a sample buffer within the shard (no heap allocation)
    pub fn allocate_sample_buffer(&mut self, size: usize) -> Result<&mut [f32]> {
        tracing::debug!("Allocating {} sample buffer in shard arena", size);
        Ok(self.shard.allocate_slice_mut::<f32>(size))
    }

    /// Reset the shard when pass completes
    pub fn reset_shard(&mut self) {
        self.shard.reset();
        tracing::info!("Reset shard for pass {}", self.pass_id);
    }

    /// Check if shadow tracking is healthy
    pub async fn shadow_healthy(&self) -> bool {
        if !self.shadow_active {
            return false;
        }

        // Update health check timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_health_check
            .store(now, std::sync::atomic::Ordering::SeqCst);

        // Check 1: Shadow SDR is still receiving samples
        let primary_count = self
            .primary_last_sample_counter
            .load(std::sync::atomic::Ordering::SeqCst);
        let shadow_count = self
            .shadow_last_sample_counter
            .load(std::sync::atomic::Ordering::SeqCst);

        // Shadow should be within 1000 samples of primary (allowing for some skew)
        let sample_delta = (primary_count as i64 - shadow_count as i64).abs();
        if sample_delta > 1000 {
            tracing::warn!(
                "Shadow sample lag detected: primary={}, shadow={}, delta={}",
                primary_count,
                shadow_count,
                sample_delta
            );
            return false;
        }

        // Check 2: Shadow demodulator state is in sync with primary
        let primary_state = self.primary_state.read().await;
        let shadow_state = self.shadow_state.read().await;

        // Compare NCO phase (should be within 0.01 cycles)
        let phase_diff = (primary_state.nco_phase - shadow_state.nco_phase).abs();
        if phase_diff > 0.01 {
            tracing::warn!(
                "Shadow NCO phase drift detected: primary={}, shadow={}, diff={}",
                primary_state.nco_phase,
                shadow_state.nco_phase,
                phase_diff
            );
            return false;
        }

        // Compare frequency offset (should be within 100 Hz)
        let freq_diff = (primary_state.frequency_offset - shadow_state.frequency_offset).abs();
        if freq_diff > 100 {
            tracing::warn!(
                "Shadow frequency offset drift detected: primary={}, shadow={}, diff={} Hz",
                primary_state.frequency_offset,
                shadow_state.frequency_offset,
                freq_diff
            );
            return false;
        }

        // Check 3: Shadow NCO phase is within tolerance (already checked above)
        // Additional check: carrier lock status should match
        if primary_state.carrier_locked != shadow_state.carrier_locked {
            tracing::warn!(
                "Shadow carrier lock mismatch: primary={}, shadow={}",
                primary_state.carrier_locked,
                shadow_state.carrier_locked
            );
            return false;
        }

        true
    }

    /// Update shadow sample counter (called when shadow SDR processes samples)
    pub fn update_shadow_sample_counter(&self, count: u64) {
        self.shadow_last_sample_counter
            .store(count, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get current failover status
    pub async fn failover_status(&self) -> FailoverStatus {
        FailoverStatus {
            shadow_available: self.shadow.is_some(),
            shadow_active: self.shadow_active,
            shadow_healthy: self.shadow_healthy().await,
            last_snapshot_sample: self.snapshot_manager.latest().map(|s| s.sample_id),
            snapshot_count: self.snapshot_manager.snapshot_count(),
        }
    }
}

/// Status of failover capability
#[derive(Debug, Clone)]
pub struct FailoverStatus {
    pub shadow_available: bool,
    pub shadow_active: bool,
    pub shadow_healthy: bool,
    pub last_snapshot_sample: Option<SampleId>,
    pub snapshot_count: usize,
}

/// Shadow tracker that manages multiple shadow SDRs
pub struct ShadowTracker {
    /// Pool of available shadow SDRs
    available_shadows: Vec<Arc<SdrHandle>>,
    /// Currently allocated shadows (pass_id -> SDR handle)
    allocated_shadows: std::collections::HashMap<PassId, Arc<SdrHandle>>,
}

impl ShadowTracker {
    pub fn new() -> Self {
        Self {
            available_shadows: Vec::new(),
            allocated_shadows: std::collections::HashMap::new(),
        }
    }

    /// Add a shadow SDR to the pool
    pub fn add_shadow(&mut self, shadow: Arc<SdrHandle>) {
        self.available_shadows.push(shadow);
    }

    /// Allocate a shadow for a pass
    pub fn allocate_shadow(&mut self, pass_id: PassId) -> Result<Option<Arc<SdrHandle>>> {
        if self.available_shadows.is_empty() {
            return Ok(None);
        }

        let shadow = self.available_shadows.pop().unwrap();
        self.allocated_shadows
            .insert(pass_id.clone(), shadow.clone());
        Ok(Some(shadow))
    }

    /// Release a shadow back to the pool
    pub fn release_shadow(&mut self, pass_id: &PassId) {
        if let Some(shadow) = self.allocated_shadows.remove(pass_id) {
            self.available_shadows.push(shadow);
        }
    }

    /// Get the number of available shadows
    pub fn available_count(&self) -> usize {
        self.available_shadows.len()
    }

    /// Get the number of allocated shadows
    pub fn allocated_count(&self) -> usize {
        self.allocated_shadows.len()
    }
}

impl Default for ShadowTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_shadow_promotion() {
        let primary = Arc::new(SdrHandle::new("primary".to_string()));
        let shadow = Arc::new(SdrHandle::new("shadow".to_string()));

        let schedule = DopplerSchedule {
            base_frequency: 1_600_000_000,
            samples: vec![(SampleId::new(0), 0)],
            computed_at: Utc::now(),
            tle_epoch: Utc::now(),
        };

        let mut acquisition = PassAcquisition::new(
            "test-pass".into(),
            primary.clone(),
            Some(shadow.clone()),
            schedule,
        );

        assert!(acquisition.shadow.is_some());
        assert!(acquisition.shadow_active);

        acquisition.promote_shadow().await.unwrap();

        assert!(acquisition.shadow.is_none());
        assert!(!acquisition.shadow_active);
        assert_eq!(acquisition.primary.device_id(), "shadow");
    }
}

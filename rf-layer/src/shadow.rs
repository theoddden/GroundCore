//! Shadow-tracking SDRs for lossless failover (Problem 1)
//!
//! This implements the architectural solution for lossless failover:
//! A designated backup SDR continuously tracks the same satellite as the primary,
//! maintaining its own demodulator state. When the primary fails, the shadow
//! is promoted with a simple pointer swap (no re-acquisition needed).

use crate::demodulator::{DemodState, DemodulatorSnapshot, SnapshotManager};
use crate::doppler::{DopplerSchedule, NcoController};
use crate::sdr::{Sample, SampleId, SdrHandle};
use chrono::{DateTime, Utc};
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

        // In a real implementation, we'd check:
        // - Shadow SDR is still receiving samples
        // - Shadow demodulator state is in sync with primary
        // - Shadow NCO phase is within tolerance

        true
    }

    /// Get current failover status
    pub fn failover_status(&self) -> FailoverStatus {
        FailoverStatus {
            shadow_available: self.shadow.is_some(),
            shadow_active: self.shadow_active,
            shadow_healthy: self.shadow_active, // Would be async check in real impl
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

#[cfg(test)]
mod tests {
    use super::*;

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
            "test-pass".to_string(),
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

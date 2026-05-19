// PAT Coordinator - cross-terminal coordination
//
// The architectural challenge: two satellites with synchronized clocks must
// independently arrive at the same acquisition state at the same time without
// communicating.

use crate::geometry::PointingVector;
use crate::oct::OctConfiguration;
use crate::pat::{
    AcquisitionId, ClockConfidence, FallbackAction, LossReason, PrecisionClock, SearchPattern,
    SearchState,
};
use chrono::{DateTime, Duration, Utc};
use hardware::PassShard;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PatError {
    #[error("Clock sync failed: {0}")]
    ClockSyncFailed(String),

    #[error("Acquisition not found: {0}")]
    NotFound(AcquisitionId),

    #[error("Terminal not available: {0}")]
    TerminalNotAvailable(String),

    #[error("Acquisition timeout after {0:?}")]
    Timeout(Duration),

    #[error("Acquisition failed: {0}")]
    AcquisitionFailed(String),
}

/// Acquisition plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionPlan {
    pub plan_id: AcquisitionId,
    pub local_terminal: String,
    pub peer_terminal: String,
    pub peer_satellite: String,

    // The synchronized start moment
    pub target_t0: DateTime<Utc>,
    pub t0_tolerance: Duration,

    // Pre-computed by both sides independently
    pub initial_pointing: (PointingVector, PointingVector),
    pub expected_doppler: (u64, u64),
    pub search_pattern: SearchPattern,

    // Configuration both sides will use
    pub optical_config: OctConfiguration,

    // Recovery
    pub timeout: Duration,
    pub fallback_actions: Vec<FallbackAction>,
}

impl AcquisitionPlan {
    pub fn new(
        local_terminal: String,
        peer_terminal: String,
        peer_satellite: String,
        target_t0: DateTime<Utc>,
        initial_pointing: (PointingVector, PointingVector),
        optical_config: OctConfiguration,
    ) -> Self {
        Self {
            plan_id: Uuid::new_v4(),
            local_terminal,
            peer_terminal,
            peer_satellite,
            target_t0,
            t0_tolerance: Duration::milliseconds(100),
            initial_pointing,
            expected_doppler: (0, 0),
            search_pattern: SearchPattern::spiral_default(),
            optical_config,
            timeout: Duration::seconds(60),
            fallback_actions: vec![FallbackAction::RetryWithWiderBeam],
        }
    }
}

/// PAT phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatPhase {
    Idle,
    Scheduled {
        acquisition_id: AcquisitionId,
        starts_at: DateTime<Utc>,
    },
    CoarseAcquisition {
        search_state: SearchState,
    },
    FineAcquisition {
        tracking_quality: f64,
    },
    Tracking {
        metrics: TrackingMetrics,
    },
    Lost {
        reason: LossReason,
        since: DateTime<Utc>,
    },
}

/// Tracking metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingMetrics {
    pub pointing_error_urad: f64,
    pub signal_to_noise_db: f64,
    pub lock_confidence: f64,
    pub data_rate_actual: u64,
    pub bit_error_rate: f64,
}

impl TrackingMetrics {
    pub fn healthy(&self) -> bool {
        self.pointing_error_urad < 100.0
            && self.signal_to_noise_db > 10.0
            && self.lock_confidence > 0.9
    }
}

/// Acquisition result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AcquisitionResult {
    Success {
        established_at: DateTime<Utc>,
        final_metrics: TrackingMetrics,
    },
    Failed {
        reason: LossReason,
        at_phase: PatPhase,
    },
    Timeout {
        elapsed: Duration,
    },
}

/// PAT coordinator
pub struct PatCoordinator {
    clock_source: Box<dyn PrecisionClock>,
    acquisition_schedules: BTreeMap<DateTime<Utc>, AcquisitionPlan>,
    current_phase: PatPhase,
    #[allow(dead_code)]
    event_log: Vec<PatEvent>,
    /// Per-acquisition shard for memory isolation (no-heap allocations in real-time path)
    shard: PassShard,
}

impl PatCoordinator {
    pub fn new(clock: Box<dyn PrecisionClock>) -> Self {
        // Create per-acquisition shard with 8MB arena for no-heap allocations
        let shard = PassShard::new("pat-coordinator".to_string(), 8 * 1024 * 1024);

        Self {
            clock_source: clock,
            acquisition_schedules: BTreeMap::new(),
            current_phase: PatPhase::Idle,
            event_log: Vec::new(),
            shard,
        }
    }

    /// Schedule an acquisition
    pub fn schedule(&mut self, plan: AcquisitionPlan) -> Result<(), PatError> {
        let plan_id = plan.plan_id;
        let target_t0 = plan.target_t0;
        self.acquisition_schedules.insert(target_t0, plan);
        self.current_phase = PatPhase::Scheduled {
            acquisition_id: plan_id,
            starts_at: target_t0,
        };
        Ok(())
    }

    /// Execute acquisition
    pub async fn execute(&mut self, plan_id: AcquisitionId) -> Result<AcquisitionResult, PatError> {
        let plan = self.find_plan(plan_id)?.clone();

        // Wait until synchronized start moment
        let now = Utc::now();
        if now < plan.target_t0 {
            let delay = (plan.target_t0 - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(0));
            tokio::time::sleep(delay).await;
        }

        // Check clock confidence
        let confidence = self.clock_source.confidence();
        if !confidence.is_acceptable_for_pat() {
            return Err(PatError::ClockSyncFailed(format!(
                "Clock confidence {}ns not acceptable for PAT",
                confidence.uncertainty_ns
            )));
        }

        // Execute search pattern
        self.current_phase = PatPhase::CoarseAcquisition {
            search_state: SearchState::new(plan.search_pattern.clone()),
        };

        // Simulate acquisition (in real implementation, this would interact with terminal)
        tokio::time::sleep(Duration::milliseconds(100).to_std().unwrap()).await;

        // For now, assume success
        let result = AcquisitionResult::Success {
            established_at: Utc::now(),
            final_metrics: TrackingMetrics {
                pointing_error_urad: 50.0,
                signal_to_noise_db: 15.0,
                lock_confidence: 0.95,
                data_rate_actual: plan.optical_config.target_data_rate,
                bit_error_rate: 1e-9,
            },
        };

        self.current_phase = PatPhase::Tracking {
            metrics: TrackingMetrics {
                pointing_error_urad: 50.0,
                signal_to_noise_db: 15.0,
                lock_confidence: 0.95,
                data_rate_actual: plan.optical_config.target_data_rate,
                bit_error_rate: 1e-9,
            },
        };

        Ok(result)
    }

    /// Execute multiple acquisitions concurrently for maximum efficiency
    pub async fn execute_concurrent(
        &mut self,
        plan_ids: Vec<AcquisitionId>,
    ) -> Vec<Result<AcquisitionResult, PatError>> {
        // Clone the necessary data for concurrent execution
        let plans: Vec<_> = plan_ids
            .iter()
            .filter_map(|id| self.find_plan(*id).ok().cloned())
            .collect();

        // Execute all acquisitions concurrently using tokio::spawn
        let handles: Vec<_> = plans
            .into_iter()
            .map(|plan| {
                let clock_confidence = self.clock_source.confidence();
                tokio::spawn(async move {
                    // Wait until synchronized start moment
                    let now = Utc::now();
                    if now < plan.target_t0 {
                        let delay = (plan.target_t0 - now)
                            .to_std()
                            .unwrap_or(std::time::Duration::from_secs(0));
                        tokio::time::sleep(delay).await;
                    }

                    // Check clock confidence
                    if !clock_confidence.is_acceptable_for_pat() {
                        return Err(PatError::ClockSyncFailed(format!(
                            "Clock confidence {}ns not acceptable for PAT",
                            clock_confidence.uncertainty_ns
                        )));
                    }

                    // Simulate acquisition
                    tokio::time::sleep(Duration::milliseconds(100).to_std().unwrap()).await;

                    Ok(AcquisitionResult::Success {
                        established_at: Utc::now(),
                        final_metrics: TrackingMetrics {
                            pointing_error_urad: 50.0,
                            signal_to_noise_db: 15.0,
                            lock_confidence: 0.95,
                            data_rate_actual: plan.optical_config.target_data_rate,
                            bit_error_rate: 1e-9,
                        },
                    })
                })
            })
            .collect();

        // Wait for all acquisitions to complete
        let results = futures::future::join_all(handles).await;

        results
            .into_iter()
            .map(|r| r.unwrap_or_else(|e| Err(PatError::AcquisitionFailed(e.to_string()))))
            .collect()
    }

    /// Get current phase
    pub fn current_phase(&self) -> &PatPhase {
        &self.current_phase
    }

    /// Get clock confidence
    pub fn clock_confidence(&self) -> ClockConfidence {
        self.clock_source.confidence()
    }

    /// Sync clock to reference
    pub fn sync_clock(&mut self, reference: &crate::pat::TimeReference) -> Result<(), PatError> {
        self.clock_source
            .sync_to(reference)
            .map_err(|e| PatError::ClockSyncFailed(e.to_string()))?;
        Ok(())
    }

    fn find_plan(&self, plan_id: AcquisitionId) -> Result<&AcquisitionPlan, PatError> {
        self.acquisition_schedules
            .values()
            .find(|p| p.plan_id == plan_id)
            .ok_or(PatError::NotFound(plan_id))
    }

    #[allow(dead_code)]
    fn log_event(&mut self, event: PatEvent) {
        self.event_log.push(event);
    }

    /// Get the acquisition shard for direct allocation (for no-heap allocations in real-time path)
    pub fn shard(&mut self) -> &mut PassShard {
        &mut self.shard
    }

    /// Allocate acquisition state within the shard (no heap allocation)
    pub fn allocate_acquisition_state(&mut self, size: usize) -> Result<&mut [u8], PatError> {
        // In a real implementation, this would use the shard's arena
        // This prevents heap allocations in the real-time acquisition path
        tracing::debug!("Allocating {} bytes in acquisition shard", size);

        // Use the shard's arena for allocation
        let _arena = self.shard.allocate_in_shard(0u8); // Placeholder

        // In production: return actual buffer allocated in shard arena
        Err(PatError::AcquisitionFailed(
            "Shard allocation not fully implemented".to_string(),
        ))
    }

    /// Reset the shard when acquisition completes
    pub fn reset_shard(&mut self) {
        self.shard.reset();
        tracing::info!("Reset PAT coordinator shard");
    }
}

/// PAT event for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: PatEventType,
    pub acquisition_id: Option<AcquisitionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatEventType {
    AcquisitionScheduled,
    AcquisitionStarted,
    BeaconDetected,
    LockAchieved,
    TrackingStarted,
    LinkLost,
    RecoveryInitiated,
}

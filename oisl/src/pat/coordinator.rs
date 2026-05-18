// PAT Coordinator - synchronized acquisition scheduling
//
// The bi-temporal connection: PAT coordination is where bi-temporal accounting
// becomes operationally essential. The acquisition protocol depends on agreed
// timestamps. The bi-temporal log proves that the timestamps were agreed correctly.

use crate::{TerminalId, AcquisitionId, OctConfiguration};
use crate::pat::{PrecisionClock, PrecisionTimestamp, ClockConfidence};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// PAT coordinator
pub struct PatCoordinator {
    clock_source: Box<dyn PrecisionClock>,
    pat_schedules: BTreeMap<DateTime<Utc>, ScheduledAcquisition>,
    event_log: Vec<PatEvent>,
}

impl PatCoordinator {
    pub fn new(clock_source: Box<dyn PrecisionClock>) -> Self {
        Self {
            clock_source,
            pat_schedules: BTreeMap::new(),
            event_log: vec![],
        }
    }

    /// Schedule acquisition for a terminal pair
    pub fn schedule_acquisition(
        &mut self,
        acquisition: ScheduledAcquisition,
    ) -> Result<(), PatError> {
        // Check clock confidence
        let confidence = self.clock_source.confidence();
        if confidence.is_degraded() {
            return Err(PatError::ClockDegraded {
                uncertainty_ns: confidence.uncertainty_ns,
            });
        }

        // Verify acquisition is in the future
        let now = self.clock_source.now().to_datetime();
        if acquisition.target_t0 <= now {
            return Err(PatError::InvalidTiming {
                message: "Acquisition time must be in the future".to_string(),
            });
        }

        // Log scheduling event
        self.log_event(PatEvent {
            event_type: PatEventType::Scheduled,
            event_time: acquisition.target_t0.into(),
            observed_at: self.clock_source.now(),
            spread: acquisition.target_t0.signed_duration_since(now),
            terminal_id: acquisition.pair.0.clone(),
            peer_terminal: acquisition.pair.1.clone(),
            operator_action_chain: vec![],
            system_decision_chain: vec![],
        });

        self.pat_schedules.insert(acquisition.target_t0, acquisition);
        Ok(())
    }

    /// Get pending acquisitions
    pub fn get_pending_acquisitions(&self) -> Vec<&ScheduledAcquisition> {
        let now = self.clock_source.now().to_datetime();
        self.pat_schedules
            .range(now..)
            .map(|(_, acq)| acq)
            .collect()
    }

    /// Get acquisitions ready to execute (within 1 second)
    pub fn get_ready_acquisitions(&self) -> Vec<&ScheduledAcquisition> {
        let now = self.clock_source.now().to_datetime();
        let window_start = now;
        let window_end = now + Duration::seconds(1);

        self.pat_schedules
            .range(window_start..window_end)
            .map(|(_, acq)| acq)
            .collect()
    }

    /// Cancel acquisition
    pub fn cancel_acquisition(&mut self, acquisition_id: AcquisitionId) -> Result<(), PatError> {
        self.pat_schedules
            .retain(|_, acq| acq.acquisition_id != acquisition_id);

        self.log_event(PatEvent {
            event_type: PatEventType::Cancelled,
            event_time: self.clock_source.now(),
            observed_at: self.clock_source.now(),
            spread: Duration::zero(),
            terminal_id: TerminalId::new_v4(),
            peer_terminal: TerminalId::new_v4(),
            operator_action_chain: vec![],
            system_decision_chain: vec![],
        });

        Ok(())
    }

    /// Get clock confidence
    pub fn clock_confidence(&self) -> ClockConfidence {
        self.clock_source.confidence()
    }

    /// Sync clock to reference
    pub fn sync_clock(&mut self, reference: &TimeReference) -> Result<(), PatError> {
        self.clock_source
            .sync_to(reference)
            .map_err(|e| PatError::ClockSyncFailed(e.to_string()))?;
        Ok(())
    }

    fn log_event(&mut self, event: PatEvent) {
        self.event_log.push(event);
    }

    /// Get event log
    pub fn event_log(&self) -> &[PatEvent] {
        &self.event_log
    }
}

/// Scheduled acquisition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledAcquisition {
    pub acquisition_id: AcquisitionId,
    pub pair: (TerminalId, TerminalId),

    // Pre-computed by both sides independently
    pub target_t0: DateTime<Utc>, // The synchronized start moment
    pub initial_pointing: (PointingVector, PointingVector),
    pub expected_doppler: (u64, u64),
    pub acquisition_sequence: AcquisitionSequence,

    // Configuration both sides will use
    pub optical_config: OctConfiguration,

    // Recovery
    pub timeout: Duration,
    pub fallback_actions: Vec<FallbackAction>,
}

/// Pointing vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointingVector {
    pub azimuth_rad: f64,
    pub elevation_rad: f64,
    pub range_km: f64,
}

/// Acquisition sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionSequence {
    pub steps: Vec<AcquisitionStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionStep {
    pub duration_ms: u64,
    pub action: AcquisitionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AcquisitionAction {
    CoarsePoint,
    FinePoint,
    BeaconTransmit,
    BeaconReceive,
    Lock,
}

/// Fallback actions for failed acquisition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallbackAction {
    RetryWithWiderBeam,
    RetryAtLaterTime(DateTime<Utc>),
    UseAlternativeTerminal(TerminalId),
    Abort,
}

/// PAT event with bi-temporal recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatEvent {
    pub event_type: PatEventType,

    // Bi-temporal: when event happened vs when system observed it
    pub event_time: PrecisionTimestamp,
    pub observed_at: PrecisionTimestamp,
    pub spread: Duration, // event_time - observed_at

    pub terminal_id: TerminalId,
    pub peer_terminal: TerminalId,

    // For audit
    pub operator_action_chain: Vec<OperatorAction>,
    pub system_decision_chain: Vec<SystemDecision>,
}

/// PAT event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatEventType {
    Scheduled,
    Started,
    Acquiring,
    Tracking,
    Lost,
    Cancelled,
    Failed,
}

/// Operator action for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorAction {
    pub operator_id: String,
    pub action: String,
    pub timestamp: DateTime<Utc>,
}

/// System decision for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDecision {
    pub decision: String,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

/// PAT error
#[derive(Debug, Clone, thiserror::Error)]
pub enum PatError {
    #[error("Clock degraded: uncertainty {uncertainty_ns} ns exceeds threshold")]
    ClockDegraded { uncertainty_ns: u64 },

    #[error("Invalid timing: {message}")]
    InvalidTiming { message: String },

    #[error("Clock sync failed: {0}")]
    ClockSyncFailed(String),

    #[error("Acquisition not found: {0}")]
    NotFound(AcquisitionId),

    #[error("Terminal not available: {0}")]
    TerminalNotAvailable(TerminalId),

    #[error("Acquisition timeout after {0:?}")]
    Timeout(Duration),
}


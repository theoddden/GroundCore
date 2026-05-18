// Control Plane Handoff - KubeSpace-inspired seamless handoff mechanism
//
// Control plane handoff with bi-temporal recording for post-incident analysis.
// Satellites maintain visibility to control plane at all times during handoff,
// ensuring uninterrupted management without container migration or Pod eviction.

use crate::{NodeId, SatelliteId, BiTemporal};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

/// Control plane handoff state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneHandoff {
    pub satellite_id: SatelliteId,
    pub source_node: NodeId,
    pub target_node: NodeId,
    pub state: HandoffState,
    pub state_history: Vec<HandoffStateTransition>,
    pub handoff_metrics: HandoffMetrics,
}

impl ControlPlaneHandoff {
    pub fn new(satellite_id: SatelliteId, source_node: NodeId, target_node: NodeId) -> Self {
        Self {
            satellite_id,
            source_node,
            target_node,
            state: HandoffState::Idle,
            state_history: vec![],
            handoff_metrics: HandoffMetrics::default(),
        }
    }

    /// Transition to new handoff state with bi-temporal recording
    pub fn transition_to(&mut self, new_state: HandoffState, trigger: HandoffTrigger) {
        let now = Utc::now();
        let transition = HandoffStateTransition {
            from_state: std::mem::replace(&mut self.state, new_state.clone()),
            to_state: new_state,
            transition_time: BiTemporal::new(now, now),
            trigger,
        };
        self.state_history.push(transition);
    }

    /// Update handoff metrics
    pub fn update_metrics(&mut self, metrics: HandoffMetrics) {
        self.handoff_metrics = metrics;
    }

    /// Get current handoff state
    pub fn current_state(&self) -> &HandoffState {
        &self.state
    }

    /// Check if handoff is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(self.state, HandoffState::Initiated | HandoffState::Processing)
    }

    /// Check if handoff is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.state, HandoffState::Complete)
    }

    /// Check if handoff failed
    pub fn is_failed(&self) -> bool {
        matches!(self.state, HandoffState::Failed { .. })
    }
}

/// Handoff state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum HandoffState {
    Idle,
    Initiated {
        initiated_at: BiTemporal<DateTime<Utc>>,
    },
    Processing {
        step: HandoffStep,
        started_at: BiTemporal<DateTime<Utc>>,
    },
    Complete {
        completed_at: BiTemporal<DateTime<Utc>>,
    },
    Failed {
        reason: String,
        failed_at: BiTemporal<DateTime<Utc>>,
    },
}

/// Handoff step during processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandoffStep {
    /// Satellite submits HandoffRequest to current control node
    RequestSubmitted,
    /// Source control node sets binding state to Releasing
    SourceReleasing,
    /// Source control node syncs configuration to target
    ConfigurationSynced,
    /// Target control node persists configuration
    TargetPersisted,
    /// Satellite loads target control node configuration
    ConfigurationLoaded,
    /// Satellite establishes control channel with target
    ChannelEstablished,
    /// Target control node sets binding state to Bound
    TargetBound,
    /// Satellite switches to new clientset
    ClientsetSwitched,
    /// Source control node sets binding state to Released
    SourceReleased,
    /// Handoff complete
    HandoffComplete,
}

/// Handoff state transition with bi-temporal recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffStateTransition {
    pub from_state: HandoffState,
    pub to_state: HandoffState,
    pub transition_time: BiTemporal<DateTime<Utc>>,
    pub trigger: HandoffTrigger,
}

/// What triggered the handoff state transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandoffTrigger {
    SatelliteInitiated,
    GeometryChange,
    ControlNodeDegradation,
    ManualCommand,
    Timeout,
    Error(String),
}

/// Handoff metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffMetrics {
    pub total_duration_ms: u64,
    pub configuration_sync_duration_ms: u64,
    pub channel_establishment_duration_ms: u64,
    pub clientset_switch_duration_ms: u64,
    pub bytes_transferred: u64,
    pub last_updated: DateTime<Utc>,
}

impl Default for HandoffMetrics {
    fn default() -> Self {
        Self {
            total_duration_ms: 0,
            configuration_sync_duration_ms: 0,
            channel_establishment_duration_ms: 0,
            clientset_switch_duration_ms: 0,
            bytes_transferred: 0,
            last_updated: Utc::now(),
        }
    }
}

/// Handoff manager for orchestrating handoff process
pub struct HandoffManager {
    active_handoffs: Vec<ControlPlaneHandoff>,
}

impl HandoffManager {
    pub fn new() -> Self {
        Self {
            active_handoffs: vec![],
        }
    }

    /// Initiate handoff for satellite
    pub fn initiate_handoff(
        &mut self,
        satellite_id: SatelliteId,
        source_node: NodeId,
        target_node: NodeId,
    ) -> Result<&mut ControlPlaneHandoff, HandoffError> {
        // Check if handoff already in progress for this satellite
        if self.active_handoffs.iter().any(|h| h.satellite_id == satellite_id && h.is_in_progress()) {
            return Err(HandoffError::HandoffAlreadyInProgress { satellite_id });
        }

        let mut handoff = ControlPlaneHandoff::new(satellite_id, source_node, target_node);
        handoff.transition_to(HandoffState::Initiated { initiated_at: BiTemporal::new(Utc::now(), Utc::now()) }, HandoffTrigger::SatelliteInitiated);
        
        self.active_handoffs.push(handoff);
        Ok(self.active_handoffs.last_mut().unwrap())
    }

    /// Advance handoff to next step
    pub fn advance_handoff(&mut self, satellite_id: &SatelliteId, next_step: HandoffStep) -> Result<(), HandoffError> {
        let handoff = self.active_handoffs
            .iter_mut()
            .find(|h| h.satellite_id == *satellite_id)
            .ok_or_else(|| HandoffError::HandoffNotFound { satellite_id: satellite_id.clone() })?;

        if !handoff.is_in_progress() {
            return Err(HandoffError::InvalidHandoffState { satellite_id: satellite_id.clone() });
        }

        let new_state = HandoffState::Processing {
            step: next_step,
            started_at: BiTemporal::new(Utc::now(), Utc::now()),
        };

        handoff.transition_to(new_state, HandoffTrigger::SatelliteInitiated);

        if next_step == HandoffStep::HandoffComplete {
            handoff.transition_to(HandoffState::Complete { completed_at: BiTemporal::new(Utc::now(), Utc::now()) }, HandoffTrigger::SatelliteInitiated);
        }

        Ok(())
    }

    /// Complete handoff with metrics
    pub fn complete_handoff(&mut self, satellite_id: &SatelliteId, metrics: HandoffMetrics) -> Result<(), HandoffError> {
        let handoff = self.active_handoffs
            .iter_mut()
            .find(|h| h.satellite_id == *satellite_id)
            .ok_or_else(|| HandoffError::HandoffNotFound { satellite_id: satellite_id.clone() })?;

        handoff.update_metrics(metrics);
        handoff.transition_to(HandoffState::Complete { completed_at: BiTemporal::new(Utc::now(), Utc::now()) }, HandoffTrigger::SatelliteInitiated);

        Ok(())
    }

    /// Fail handoff
    pub fn fail_handoff(&mut self, satellite_id: &SatelliteId, reason: String) -> Result<(), HandoffError> {
        let handoff = self.active_handoffs
            .iter_mut()
            .find(|h| h.satellite_id == *satellite_id)
            .ok_or_else(|| HandoffError::HandoffNotFound { satellite_id: satellite_id.clone() })?;

        handoff.transition_to(HandoffState::Failed { reason: reason.clone(), failed_at: BiTemporal::new(Utc::now(), Utc::now()) }, HandoffTrigger::Error(reason));

        Ok(())
    }

    /// Get handoff for satellite
    pub fn get_handoff(&self, satellite_id: &SatelliteId) -> Option<&ControlPlaneHandoff> {
        self.active_handoffs.iter().find(|h| h.satellite_id == *satellite_id)
    }

    /// Get all active handoffs
    pub fn get_active_handoffs(&self) -> &[ControlPlaneHandoff] {
        &self.active_handoffs
    }

    /// Clean up completed handoffs
    pub fn cleanup_completed(&mut self) {
        self.active_handoffs.retain(|h| !h.is_complete());
    }
}

impl Default for HandoffManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handoff error
#[derive(Debug, Clone, thiserror::Error)]
pub enum HandoffError {
    #[error("Handoff already in progress for satellite: {satellite_id}")]
    HandoffAlreadyInProgress { satellite_id: SatelliteId },

    #[error("Handoff not found for satellite: {satellite_id}")]
    HandoffNotFound { satellite_id: SatelliteId },

    #[error("Invalid handoff state for satellite: {satellite_id}")]
    InvalidHandoffState { satellite_id: SatelliteId },

    #[error("Handoff step failed: {0}")]
    StepFailed(String),

    #[error("Configuration sync failed: {0}")]
    ConfigurationSyncFailed(String),

    #[error("Channel establishment failed: {0}")]
    ChannelEstablishmentFailed(String),
}

// Link recovery and failover
//
// When a primary terminal fails mid-acquisition or mid-tracking, the system needs
// to recover to a backup terminal without losing link state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type LinkId = Uuid;

/// Failover strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailoverStrategy {
    /// Switch to backup terminal on same satellite
    SwitchTerminal { backup_terminal_id: String },

    /// Re-route through different path
    Reroute { alternative_path: Vec<String> },

    /// Drop and re-establish from scratch
    Reacquire,

    /// Abort and report failure
    Abort,
}

/// Handoff strategy for pre-emptive link handoffs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandoffStrategy {
    /// Seamless handoff with zero packet loss
    Seamless { overlap_ms: u64 },

    /// Make-before-break handoff
    MakeBeforeBreak { overlap_ms: u64 },

    /// Break-before-make handoff
    BreakBeforeMake { gap_ms: u64 },

    /// No handoff - let link fail naturally
    NoHandoff,
}

impl HandoffStrategy {
    pub fn seamless(overlap_ms: u64) -> Self {
        Self::Seamless { overlap_ms }
    }

    pub fn make_before_break(overlap_ms: u64) -> Self {
        Self::MakeBeforeBreak { overlap_ms }
    }

    pub fn break_before_make(gap_ms: u64) -> Self {
        Self::BreakBeforeMake { gap_ms }
    }

    pub fn no_handoff() -> Self {
        Self::NoHandoff
    }
}

/// Handoff execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffExecution {
    pub handoff_id: Uuid,
    pub link_id: LinkId,
    pub strategy: HandoffStrategy,
    pub initiated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: bool,
    pub packets_lost: u64,
}

impl HandoffExecution {
    pub fn new(link_id: LinkId, strategy: HandoffStrategy) -> Self {
        Self {
            handoff_id: Uuid::new_v4(),
            link_id,
            strategy,
            initiated_at: Utc::now(),
            completed_at: None,
            success: false,
            packets_lost: 0,
        }
    }

    pub fn complete(&mut self, success: bool, packets_lost: u64) {
        self.completed_at = Some(Utc::now());
        self.success = success;
        self.packets_lost = packets_lost;
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.completed_at
            .map(|t| (t - self.initiated_at).num_milliseconds() as u64)
    }
}

/// Failover manager
pub struct FailoverManager {
    active_handoffs: Vec<HandoffExecution>,
}

impl FailoverManager {
    pub fn new() -> Self {
        Self {
            active_handoffs: Vec::new(),
        }
    }

    pub fn initiate_handoff(
        &mut self,
        link_id: LinkId,
        strategy: HandoffStrategy,
    ) -> HandoffExecution {
        let execution = HandoffExecution::new(link_id, strategy);
        self.active_handoffs.push(execution.clone());
        execution
    }

    pub fn complete_handoff(
        &mut self,
        handoff_id: Uuid,
        success: bool,
        packets_lost: u64,
    ) -> Result<(), FailoverError> {
        if let Some(execution) = self
            .active_handoffs
            .iter_mut()
            .find(|h| h.handoff_id == handoff_id)
        {
            execution.complete(success, packets_lost);
            Ok(())
        } else {
            Err(FailoverError::HandoffNotFound(handoff_id))
        }
    }

    pub fn get_handoff(&self, handoff_id: Uuid) -> Option<&HandoffExecution> {
        self.active_handoffs
            .iter()
            .find(|h| h.handoff_id == handoff_id)
    }

    pub fn cleanup_completed(&mut self) {
        self.active_handoffs.retain(|h| h.completed_at.is_none());
    }
}

impl Default for FailoverManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FailoverError {
    #[error("Handoff not found: {0}")]
    HandoffNotFound(Uuid),

    #[error("Handoff already completed")]
    AlreadyCompleted,
}

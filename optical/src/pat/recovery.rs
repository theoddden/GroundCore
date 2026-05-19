// Re-acquisition after loss
//
// When a link is lost, the system needs to recover. Recovery strategies range
// from simple retries to complete re-scheduling with different parameters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Loss reason
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LossReason {
    PatTimeout,
    SignalDegraded,
    PowerLoss,
    MechanicalFailure,
    ClockDesync,
    AtmosphericTurbulence,
    PointingDrift,
    Unknown,
}

/// Recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry with wider beam divergence
    RetryWithWiderBeam { new_divergence_mrad: f64 },

    /// Retry at a later time when geometry is more favorable
    RetryAtLaterTime { retry_at: DateTime<Utc> },

    /// Use an alternative terminal on the same satellite
    UseAlternativeTerminal { terminal_id: String },

    /// Abort and report failure
    Abort,

    /// Full re-acquisition with new search pattern
    FullReacquisition { new_search_pattern: String },
}

impl RecoveryStrategy {
    pub fn for_loss_reason(reason: &LossReason) -> Self {
        match reason {
            LossReason::PatTimeout => Self::RetryWithWiderBeam {
                new_divergence_mrad: 0.2,
            },
            LossReason::SignalDegraded => Self::FullReacquisition {
                new_search_pattern: "raster".to_string(),
            },
            LossReason::AtmosphericTurbulence => Self::RetryAtLaterTime {
                retry_at: Utc::now() + chrono::Duration::minutes(10),
            },
            LossReason::PointingDrift => Self::FullReacquisition {
                new_search_pattern: "spiral".to_string(),
            },
            LossReason::ClockDesync => Self::RetryAtLaterTime {
                retry_at: Utc::now() + chrono::Duration::seconds(30),
            },
            _ => Self::Abort,
        }
    }
}

/// Fallback action for failed acquisition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallbackAction {
    RetryWithWiderBeam,
    RetryAtLaterTime(DateTime<Utc>),
    UseAlternativeTerminal(String),
    Abort,
}

impl FallbackAction {
    pub fn to_recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            Self::RetryWithWiderBeam => RecoveryStrategy::RetryWithWiderBeam {
                new_divergence_mrad: 0.2,
            },
            Self::RetryAtLaterTime(dt) => RecoveryStrategy::RetryAtLaterTime { retry_at: *dt },
            Self::UseAlternativeTerminal(id) => RecoveryStrategy::UseAlternativeTerminal {
                terminal_id: id.clone(),
            },
            Self::Abort => RecoveryStrategy::Abort,
        }
    }
}

/// Recovery attempt result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    pub attempt_number: u32,
    pub strategy: RecoveryStrategy,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: bool,
    pub duration_ms: Option<u64>,
}

impl RecoveryAttempt {
    pub fn new(attempt_number: u32, strategy: RecoveryStrategy) -> Self {
        Self {
            attempt_number,
            strategy,
            started_at: Utc::now(),
            completed_at: None,
            success: false,
            duration_ms: None,
        }
    }

    pub fn complete(&mut self, success: bool) {
        self.completed_at = Some(Utc::now());
        self.success = success;
        if let Some(completed) = self.completed_at {
            self.duration_ms = Some((completed - self.started_at).num_milliseconds() as u64);
        }
    }
}

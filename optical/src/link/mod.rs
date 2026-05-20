// Link lifecycle and state
//
// Link lifecycle management for optical links includes state machine,
// metrics tracking, degradation detection, and failover/handoff.

pub mod degradation;
pub mod failover;
pub mod metrics;
pub mod scintillation;
pub mod state_machine;

pub use degradation::DegradationDetector;
pub use failover::{FailoverManager, FailoverStrategy, HandoffExecution, HandoffStrategy};
pub use metrics::{BoundedHistory, MetricSnapshot};
pub use scintillation::{AtmosphericSite, ScintillationModel, ScintillationSeverity, SnrDistribution};
pub use state_machine::{
    DegradationAction, DegradationReason, FailureCause, LinkMetrics, LinkPhase, LinkQuality,
    OpticalLink, RecoveryStrategy, TerminationReason,
};

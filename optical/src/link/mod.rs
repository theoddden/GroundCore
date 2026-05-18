// Link lifecycle and state
//
// Link lifecycle management for optical links includes state machine,
// metrics tracking, degradation detection, and failover/handoff.

pub mod state_machine;
pub mod metrics;
pub mod degradation;
pub mod failover;

pub use state_machine::{
    LinkPhase, LinkQuality, DegradationReason, DegradationAction, RecoveryStrategy,
    TerminationReason, FailureCause, OpticalLink, LinkMetrics,
};
pub use metrics::{MetricSnapshot, BoundedHistory};
pub use degradation::DegradationDetector;
pub use failover::{FailoverStrategy, HandoffStrategy, HandoffExecution, FailoverManager};

//! Embedded Claude Code agent for anomaly synthesis
//!
//! This implements Problem 6: Agent-mediated anomaly detection.
//!
//! The agent has structured access to read the system's state and propose actions:
//! - Watching for anomalies that don't fit normal patterns
//! - Synthesizing thousands of weak signals into coherent alerts
//! - Explaining decisions when operators ask "why did this happen?"
//! - Tuning the scheduler over time by proposing improvements
//! - Drafting incident reports when something goes wrong
//! - Answering operator questions in natural language
//!
//! For routine actions the agent can act autonomously (with logging).
//! For consequential actions it proposes and waits for operator approval.

pub mod observation;
pub mod synthesis;
pub mod proposal;
pub mod context;

pub use observation::{ObservationTool, SystemState, TelemetrySnapshot};
pub use synthesis::{AnomalySynthesizer, AnomalyAlert, WeakSignal};
pub use proposal::{ActionProposal, ProposalType, ApprovalStatus};
pub use context::{AgentContext, QueryResult};

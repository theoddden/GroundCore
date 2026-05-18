//! Pass-isolated process supervision for zero-downtime deployment
//!
//! This implements Problem 7: Continuous operation through code deployment.
//!
//! Architectural decomposition:
//! - Each active pass runs in its own subprocess
//! - Supervised by a long-lived parent process
//! - Code deployment swaps the parent, pass subprocesses continue under old code
//! - New passes use new code
//! - No shared mutable state between passes (enforced by sharding)
//! - Communication through versioned shared memory

pub mod supervisor;
pub mod subprocess;
pub mod handoff;
pub mod version;

pub use supervisor::{Supervisor, SupervisorConfig};
pub use subprocess::{PassProcess, ProcessState, ProcessHandle};
pub use handoff::{HandoffManager, HandoffResult};
pub use version::{Version, VersionedBinary};

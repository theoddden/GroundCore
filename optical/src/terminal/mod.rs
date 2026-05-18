// Vendor abstraction
//
// The optical layer talks to all four SDA-qualified vendors through one trait.
// Each vendor implementation wraps the vendor's specific management interface.

pub mod capability;
pub mod safe_mode;
pub mod telemetry;
pub mod trait_def;

pub use capability::{FieldOfRegard, PowerProfile, TerminalCapability, Vendor};
pub use safe_mode::{RecoveryProcedure, SafeModeState};
pub use telemetry::{HealthReport, TelemetryFrame, TelemetryStream};
pub use trait_def::{OpticalTerminal, TerminalError, TerminalStatus};

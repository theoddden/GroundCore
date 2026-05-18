// Vendor abstraction
//
// The optical layer talks to all four SDA-qualified vendors through one trait.
// Each vendor implementation wraps the vendor's specific management interface.

pub mod trait_def;
pub mod capability;
pub mod telemetry;
pub mod safe_mode;

pub use trait_def::{OpticalTerminal, TerminalStatus, TerminalError};
pub use capability::{TerminalCapability, Vendor, PowerProfile, FieldOfRegard};
pub use telemetry::{TelemetryStream, TelemetryFrame, HealthReport};
pub use safe_mode::{SafeModeState, RecoveryProcedure};

//! Uplink Command & Control
//!
//! This crate provides command and control functionality for satellite uplink:
//! - Command representation and validation
//! - Encryption and authentication with pluggable crypto backends
//! - Command queuing with priority and persistence
//! - Transmission scheduling with pass window integration
//! - Confirmation tracking with ACK/NACK and retry logic

pub mod command;
pub mod confirmation;
pub mod crypto;
pub mod queue;
pub mod scheduler;
pub mod transmitter;

pub use command::{Command, CommandId, CommandPriority, CommandState, CommandType};
pub use confirmation::{CommandOutcome, ConfirmationTracker};
pub use crypto::{AesGcmBackend, CryptoBackend, CryptoError};
pub use queue::{CommandQueue, QueuedCommand};
pub use scheduler::{TransmissionScheduler, TransmissionWindow};
pub use transmitter::{Transmitter, TransmitterId, TransmitterStatus};

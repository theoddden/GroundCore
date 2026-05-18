//! Bi-temporal logging with sample provenance
//!
//! This provides the foundational bi-temporal logging primitive used throughout
//! Ground Station Core. Every sample is logged with two timestamps:
//! - Event time: when the satellite emitted the signal (computed from orbital position)
//! - Reception time: when the ground station received the signal
//!
//! This bi-temporal logging makes everything provable later — you can prove exactly
//! what you received and exactly when. It's critical for:
//! - Federation verification (Problem 4)
//! - Reputation computation (Problem 3)
//! - Regulatory compliance (Problem 5)
//! - Incident analysis and debugging

pub mod timestamp;
pub mod log;
pub mod provenance;
pub mod snapshot;

pub use timestamp::{BiTemporal, EventTime, ReceptionTime};
pub use log::{BitemporalLog, LogEntry, LogEntryType};
pub use provenance::{ProvenanceChain, ProvenanceVerifier};
pub use snapshot::{LogSnapshot, SnapshotManager};

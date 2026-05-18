//! Snapshotting infrastructure for state persistence and exchange
//!
//! This provides snapshotting primitives for:
//! - Schedule snapshots for incremental re-optimization
//! - Demodulator state for failover recovery
//! - Federation state exchange for cross-station verification

pub mod schedule;
pub mod demodulator;
pub mod federation;
pub mod manager;

pub use schedule::{ScheduleSnapshot, ScheduleSnapshotter};
pub use demodulator::{DemodulatorSnapshotWrapper as DemodulatorSnapshot, DemodulatorSnapshotter};
pub use federation::{FederationSnapshot, FederationSnapshotter};
pub use manager::{SnapshotManager, SnapshotMetadata};

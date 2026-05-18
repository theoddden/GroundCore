//! Batching infrastructure for Ground Station Core
//!
//! This provides batching primitives for:
//! - TLE refresh (atomic constellation state updates)
//! - Demodulator output to Float Protocols
//! - Log entry batching for efficient storage

pub mod demodulator;
pub mod log;
pub mod tle;

pub use demodulator::{DemodulatorBatch, DemodulatorBatcher};
pub use log::{LogBatch, LogBatcher};
pub use tle::{TleBatch, TleBatcher, TleData};

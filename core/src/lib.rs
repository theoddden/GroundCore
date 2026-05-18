//! Ground Station Core - Central types and shared utilities
//!
//! This crate provides the foundational types, error handling, and shared
//! utilities used across all layers of the ground station system.

pub mod error;
pub mod types;

pub use error::{GroundStationError, Result};
pub use types::*;

//! Simulated SDR backend for testing
//!
//! This provides a realistic SDR simulation for testing the RF layer without
//! requiring actual hardware. It can simulate:
//! - Doppler shift over time
//! - Signal fading
//! - Hardware failures
//! - Various noise conditions
//!
//! This is critical for testing shadow failover, Doppler correction, and
//! other real-time features in a controlled environment.

pub mod simulator;
pub mod signal;
pub mod failure;

pub use simulator::{SimulatedSdr, SimulatedSdrConfig};
pub use signal::{SignalGenerator, SignalType, NoiseModel};
pub use failure::{FailureInjector, FailureType};

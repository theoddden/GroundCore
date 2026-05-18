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

pub mod failure;
pub mod signal;
pub mod simulator;

pub use failure::{FailureInjector, FailureType};
pub use signal::{NoiseModel, SignalGenerator, SignalType};
pub use simulator::{SimulatedSdr, SimulatedSdrConfig};

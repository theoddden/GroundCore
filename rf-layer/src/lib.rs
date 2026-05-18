//! RF Layer - Real-time signal processing with zero allocations
//!
//! This crate implements the kinetic plane of the ground station system:
//! - Shadow-tracking SDRs for lossless failover (Problem 1)
//! - Predictive NCO programming with phase-continuous Doppler correction (Problem 2)
//! - Demodulator state management with bi-temporal provenance
//!
//! All code in the real-time path uses no-alloc guarantees to prevent
//! memory allocation pauses that would lose samples.

pub mod demodulator;
pub mod doppler;
pub mod sdr;
pub mod shadow;

pub use demodulator::{DemodState, DemodulatorSnapshot};
pub use doppler::{DopplerSchedule, NcoController};
pub use sdr::{Sample, SampleId, SdrHandle};
pub use shadow::{PassAcquisition, ShadowTracker};

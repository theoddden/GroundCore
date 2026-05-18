//! Satellite tracking with SGP4 propagation and UKF refinement
//!
//! This implements satellite position prediction using SGP4, with refinement
//! via Unscented Kalman Filter based on observed Doppler residuals.
//! This addresses the accuracy limitation mentioned in Problem 2.

pub mod propagation;
pub mod tle;
pub mod ukf;

pub use propagation::{OrbitalState, PropagationCache, Propagator};
pub use tle::{TleData, TleSet};
pub use ukf::{UkfRefiner, UkfState};

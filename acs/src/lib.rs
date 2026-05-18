//! Antenna Control System (ACS)
//!
//! This crate provides antenna control functionality:
//! - Antenna controller abstraction
//! - Tracking algorithms (open-loop, closed-loop, predictive)
//! - Slew rate limiting and motion planning
//! - Homing procedures and safety interlocks

pub mod controller;
pub mod homing;
pub mod safety;
pub mod slew;
pub mod tracking;
pub mod vendor;

pub use controller::{AntennaController, ControllerError, ControllerStatus, PointingTarget};
pub use homing::{CalibrationResult, HomingProcedure, StowPosition};
pub use safety::{EmergencyStop, InterlockState, SafetyInterlock};
pub use slew::{MotionProfile, SlewId, SlewLimiter, SlewTrajectory};
pub use tracking::{TrackingAlgorithm, TrackingCorrection, TrackingFeedback, TrackingMode};

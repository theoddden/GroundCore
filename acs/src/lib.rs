//! Antenna Control System (ACS)
//!
//! This crate provides antenna control functionality:
//! - Antenna controller abstraction
//! - Tracking algorithms (open-loop, closed-loop, predictive)
//! - Slew rate limiting and motion planning
//! - Homing procedures and safety interlocks

pub mod controller;
pub mod tracking;
pub mod slew;
pub mod homing;
pub mod safety;
pub mod vendor;

pub use controller::{AntennaController, ControllerStatus, PointingTarget, ControllerError};
pub use tracking::{TrackingAlgorithm, TrackingMode, TrackingFeedback, TrackingCorrection};
pub use slew::{SlewLimiter, SlewTrajectory, SlewId, MotionProfile};
pub use homing::{HomingProcedure, StowPosition, CalibrationResult};
pub use safety::{SafetyInterlock, InterlockState, EmergencyStop};

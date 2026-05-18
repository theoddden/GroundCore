//! RF Front-End Control
//!
//! This crate provides RF front-end control functionality:
//! - RF device abstraction
//! - Amplifier control with gain staging and thermal protection
//! - Filter control with switching and tuning
//! - RF switch control
//! - Attenuator control
//! - AGC controller with automatic gain adjustment

pub mod agc;
pub mod amplifier;
pub mod attenuator;
pub mod device;
pub mod filter;
pub mod switch;
pub mod vendor;

pub use agc::{AgcConfig, AgcController, AgcMode};
pub use amplifier::{AmplifierControl, AmplifierStatus, GainStage};
pub use attenuator::{AttenuationLevel, AttenuatorControl};
pub use device::{RfDevice, RfDeviceState, RfDeviceType};
pub use filter::{FilterControl, FilterId, FilterSpec, FilterType};
pub use switch::{SwitchControl, SwitchPosition, SwitchState};

//! RF Front-End Control
//!
//! This crate provides RF front-end control functionality:
//! - RF device abstraction
//! - Amplifier control with gain staging and thermal protection
//! - Filter control with switching and tuning
//! - RF switch control
//! - Attenuator control
//! - AGC controller with automatic gain adjustment

pub mod device;
pub mod amplifier;
pub mod filter;
pub mod switch;
pub mod attenuator;
pub mod agc;
pub mod vendor;

pub use device::{RfDevice, RfDeviceType, RfDeviceState};
pub use amplifier::{AmplifierControl, GainStage, AmplifierStatus};
pub use filter::{FilterControl, FilterSpec, FilterId, FilterType};
pub use switch::{SwitchControl, SwitchState, SwitchPosition};
pub use attenuator::{AttenuatorControl, AttenuationLevel};
pub use agc::{AgcController, AgcMode, AgcConfig};

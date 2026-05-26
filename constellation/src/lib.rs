//! Constellation Digital Twin - Multi-satellite orbital state management
//!
//! This crate provides constellation-scale satellite propagation, pass prediction,
//! and integration with SDR simulation, optical tracking, and OISL routing.
//!
//! # Features
//! - Multi-constellation management (Starlink, OneWeb, GPS, etc.)
//! - Ground station pass prediction
//! - ISL geometry forecasting for OISL
//! - Integration with sdr-sim, optical, and oisl crates

pub mod manager;
pub mod pass;
pub mod satellite;

#[cfg(feature = "oisl-integration")]
pub mod oisl_integration;

#[cfg(feature = "optical-integration")]
pub mod optical_integration;

pub use manager::{ConstellationManager, ConstellationSnapshot};
pub use pass::{Pass, PassPredictor, PassWindow};
pub use satellite::Satellite;

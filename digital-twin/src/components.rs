//! ECS components for digital twin simulation
//!
//! This module defines the Bevy ECS components that represent satellite and ground
//! station state in the digital twin. Each component corresponds to a physical
//! aspect of the system that can be simulated in parallel.

use bevy_ecs::component::Component;
use chrono::{DateTime, Utc};
use ground_core::{LinkId, SatelliteId, StationId};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use sgp4::Elements;
use std::time::Instant;

/// Orbital state component for satellite entities
#[derive(Component, Debug, Clone)]
pub struct OrbitalState {
    /// SGP4 orbital elements
    pub sgp4_elements: Elements,
    /// When this orbital state was last updated
    pub last_updated: Instant,
}

impl OrbitalState {
    /// Create a new orbital state from SGP4 elements
    pub fn new(sgp4_elements: Elements) -> Self {
        Self {
            sgp4_elements,
            last_updated: Instant::now(),
        }
    }

    /// Mark the orbital state as updated
    pub fn mark_updated(&mut self) {
        self.last_updated = Instant::now();
    }
}

/// Position component (ECI coordinates)
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Position in ECI coordinates (km)
    pub position: Vector3<f64>,
    /// Velocity in ECI coordinates (km/s)
    pub velocity: Vector3<f64>,
    /// When this position is valid
    pub time: DateTime<Utc>,
}

impl Position {
    /// Create a new position
    pub fn new(position: Vector3<f64>, velocity: Vector3<f64>, time: DateTime<Utc>) -> Self {
        Self {
            position,
            velocity,
            time,
        }
    }
}

/// Optical terminal state component
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct OpticalTerminalState {
    /// Terminal vendor (e.g., "Mynaric", "Tesat")
    pub vendor: String,
    /// Current link phase
    pub phase: LinkPhase,
    /// Pointing vector (azimuth, elevation in degrees)
    pub pointing: PointingVector,
    /// Terminal identifier
    pub terminal_id: String,
}

impl OpticalTerminalState {
    /// Create a new optical terminal state
    pub fn new(vendor: String, terminal_id: String) -> Self {
        Self {
            vendor,
            phase: LinkPhase::Idle,
            pointing: PointingVector::default(),
            terminal_id,
        }
    }
}

/// Link phase for optical terminals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkPhase {
    /// Terminal is idle
    Idle,
    /// Pointing acquisition in progress
    Acquisition,
    /// Tracking established
    Tracking,
    /// Link degraded
    Degraded,
    /// Link failed
    Failed,
}

/// Pointing vector (azimuth, elevation)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PointingVector {
    /// Azimuth angle (degrees, 0-360)
    pub azimuth: f64,
    /// Elevation angle (degrees, -90 to 90)
    pub elevation: f64,
}

impl Default for PointingVector {
    fn default() -> Self {
        Self {
            azimuth: 0.0,
            elevation: 0.0,
        }
    }
}

/// Link budget component (with bounded intervals)
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct LinkBudget {
    /// Path loss (dB) as an interval
    pub path_loss_db: crate::divergence::Interval,
    /// Atmospheric attenuation (dB) as an interval
    pub atmospheric_attenuation_db: crate::divergence::Interval,
    /// Predicted SNR (dB) as an interval
    pub predicted_snr: crate::divergence::Interval,
    /// Link identifier
    pub link_id: LinkId,
}

impl LinkBudget {
    /// Create a new link budget
    pub fn new(link_id: LinkId) -> Self {
        Self {
            path_loss_db: crate::divergence::Interval::new(0.0, 0.0),
            atmospheric_attenuation_db: crate::divergence::Interval::new(0.0, 0.0),
            predicted_snr: crate::divergence::Interval::new(0.0, 0.0),
            link_id,
        }
    }

    /// Update the link budget with new intervals
    pub fn update(
        &mut self,
        path_loss: crate::divergence::Interval,
        atmospheric: crate::divergence::Interval,
        snr: crate::divergence::Interval,
    ) {
        self.path_loss_db = path_loss;
        self.atmospheric_attenuation_db = atmospheric;
        self.predicted_snr = snr;
    }
}

/// Ground station component
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct GroundStation {
    /// Station identifier
    pub station_id: StationId,
    /// Latitude (degrees)
    pub lat: f64,
    /// Longitude (degrees)
    pub lon: f64,
    /// Altitude (meters)
    pub alt: f64,
    /// Terminal configuration
    pub terminal_config: StationTerminalConfig,
}

impl GroundStation {
    /// Create a new ground station
    pub fn new(station_id: StationId, lat: f64, lon: f64, alt: f64) -> Self {
        Self {
            station_id,
            lat,
            lon,
            alt,
            terminal_config: StationTerminalConfig::default(),
        }
    }
}

/// Station terminal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationTerminalConfig {
    /// Has optical terminal
    pub has_optical: bool,
    /// Has RF terminal
    pub has_rf: bool,
    /// Optical terminal vendor (if applicable)
    pub optical_vendor: Option<String>,
}

impl Default for StationTerminalConfig {
    fn default() -> Self {
        Self {
            has_optical: false,
            has_rf: true,
            optical_vendor: None,
        }
    }
}

/// Satellite identifier component
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteIdComponent {
    /// Satellite identifier
    pub satellite_id: SatelliteId,
    /// NORAD catalog number
    pub norad_id: u32,
    /// Satellite name
    pub name: String,
}

impl SatelliteIdComponent {
    /// Create a new satellite ID component
    pub fn new(satellite_id: SatelliteId, norad_id: u32, name: String) -> Self {
        Self {
            satellite_id,
            norad_id,
            name,
        }
    }
}

/// Visibility component for satellite-ground station pairs
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Visibility {
    /// Whether the satellite is visible from the ground station
    pub is_visible: bool,
    /// Elevation angle (degrees)
    pub elevation: f64,
    /// Range (km)
    pub range: f64,
    /// Visibility window start
    pub window_start: Option<DateTime<Utc>>,
    /// Visibility window end
    pub window_end: Option<DateTime<Utc>>,
}

impl Visibility {
    /// Create a new visibility state
    pub fn new() -> Self {
        Self {
            is_visible: false,
            elevation: 0.0,
            range: 0.0,
            window_start: None,
            window_end: None,
        }
    }

    /// Check if currently in visibility window
    pub fn in_window(&self, time: DateTime<Utc>) -> bool {
        if let (Some(start), Some(end)) = (self.window_start, self.window_end) {
            time >= start && time <= end
        } else {
            false
        }
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Self::new()
    }
}

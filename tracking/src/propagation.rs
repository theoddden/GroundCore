//! Orbital propagation with caching
//!
//! This implements orbital state propagation with caching to avoid redundant
/// SGP4 computations. The cache stores anchor points for interpolation.

pub use caching::PropagationCache as SharedPropagationCache;
use crate::tle::TleData;
use chrono::{DateTime, Utc};
use ground_core::{Result, SatelliteId};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Orbital state at a specific time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitalState {
    /// Position in ECI coordinates (meters)
    pub position: Vector3<f64>,
    /// Velocity in ECI coordinates (m/s)
    pub velocity: Vector3<f64>,
    /// When this state is valid
    pub time: DateTime<Utc>,
    /// TLE epoch used for propagation
    pub tle_epoch: DateTime<Utc>,
}

impl OrbitalState {
    /// Compute range from a ground station
    pub fn range_from_station(&self, station_lat: f64, station_lon: f64, station_alt: f64) -> f64 {
        // Convert station lat/lon/alt to ECI
        let station_eci = geodetic_to_eci(station_lat, station_lon, station_alt, self.time);
        
        // Compute range
        (self.position - station_eci).norm()
    }
    
    /// Compute range rate (derivative of range) from a ground station
    pub fn range_rate_from_station(&self, station_lat: f64, station_lon: f64, station_alt: f64) -> f64 {
        let station_eci = geodetic_to_eci(station_lat, station_lon, station_alt, self.time);
        let rel_pos = self.position - station_eci;
        let rel_vel = self.velocity; // Station velocity is negligible for this calculation
        
        // Range rate = (rel_pos . rel_vel) / |rel_pos|
        rel_pos.dot(&rel_vel) / rel_pos.norm()
    }
}

/// Convert geodetic coordinates to ECI
fn geodetic_to_eci(lat: f64, lon: f64, alt: f64, time: DateTime<Utc>) -> Vector3<f64> {
    // Earth parameters
    let a = 6378137.0; // Semi-major axis (meters)
    let e2 = 0.00669437999014; // Eccentricity squared
    
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    
    // Compute Earth rotation angle at given time
    let theta = gmst(time);
    
    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let sin_lon = (lon_rad + theta).sin();
    let cos_lon = (lon_rad + theta).cos();
    
    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let x = (n + alt) * cos_lat * cos_lon;
    let y = (n + alt) * cos_lat * sin_lon;
    let z = (n * (1.0 - e2) + alt) * sin_lat;
    
    Vector3::new(x, y, z)
}

/// Greenwich Mean Sidereal Time
fn gmst(time: DateTime<Utc>) -> f64 {
    // Simplified GMST calculation
    // In production, use a more precise algorithm
    let j2000 = DateTime::from_timestamp(946684800, 0)
        .unwrap()
        .with_timezone(&Utc);
    let days_since_j2000 = (time - j2000).num_seconds() as f64 / 86400.0;
    let gmst = 1.753368559 + 0.017202791805 * days_since_j2000;
    gmst % (2.0 * std::f64::consts::PI)
}

/// SGP4 Propagator
pub struct Propagator {
    /// SGP4 elements for each satellite with their TLE epoch
    elements: HashMap<SatelliteId, (sgp4::Elements, DateTime<Utc>)>,
}

impl Propagator {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }
    
    /// Load TLE into the propagator
    pub fn load_tle(&mut self, tle: &TleData) -> Result<()> {
        let elements = sgp4::Elements::from_tle(None, tle.line1.as_bytes(), tle.line2.as_bytes())
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 parse error: {}", e)))?;
        
        self.elements.insert(tle.satellite_id.clone(), (elements, tle.epoch));
        Ok(())
    }
    
    /// Propagate to a specific time
    pub fn propagate(&self, satellite_id: &SatelliteId, time: DateTime<Utc>) -> Result<OrbitalState> {
        let (elements, tle_epoch) = self
            .elements
            .get(satellite_id)
            .ok_or_else(|| ground_core::GroundStationError::Tracking(format!("Satellite {} not loaded", satellite_id)))?;
        
        // Compute minutes from epoch
        let minutes_since_epoch = (time - tle_epoch).num_seconds() as f64 / 60.0;
        
        let constants = sgp4::Constants::from_elements(elements)
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 constants error: {}", e)))?;
        
        let state = constants
            .propagate(sgp4::MinutesSinceEpoch(minutes_since_epoch))
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 propagation error: {}", e)))?;
        
        Ok(OrbitalState {
            position: Vector3::new(state.position[0], state.position[1], state.position[2]),
            velocity: Vector3::new(state.velocity[0], state.velocity[1], state.velocity[2]),
            time,
            tle_epoch: *tle_epoch,
        })
    }
}

// Re-export the shared PropagationCache from the caching crate
// This eliminates code duplication and ensures consistency
pub use SharedPropagationCache as PropagationCache;

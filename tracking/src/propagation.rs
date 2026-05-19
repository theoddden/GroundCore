//! Orbital propagation with caching
//!
//! This implements orbital state propagation with caching to avoid redundant
use crate::tle::TleData;
/// SGP4 computations. The cache stores anchor points for interpolation.
pub use caching::PropagationCache as SharedPropagationCache;
use chrono::{DateTime, Utc};
use ground_core::{Result, SatelliteId};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::PI;

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
    pub fn range_rate_from_station(
        &self,
        station_lat: f64,
        station_lon: f64,
        station_alt: f64,
    ) -> f64 {
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

/// Greenwich Mean Sidereal Time for a given UTC instant (radians).
/// Uses IAU 1982 mean sidereal time formula (matches ukf.rs implementation).
fn gmst(time: DateTime<Utc>) -> f64 {
    // Julian date of J2000.0 epoch = 2451545.0
    let jd = 2_451_545.0 + (time.timestamp() as f64 - 946_727_935.816) / 86_400.0;
    let t_ut1 = (jd - 2_451_545.0) / 36_525.0;
    // GMST in seconds
    let gmst_sec =
        67_310.548_41 + (8_640_184.812_866 + (0.093_104 - 6.2e-6 * t_ut1) * t_ut1) * t_ut1;
    (gmst_sec * PI / 43_200.0).rem_euclid(2.0 * PI)
}

/// SGP4 Propagator
pub struct Propagator {
    /// SGP4 elements for each satellite with their TLE epoch
    elements: HashMap<SatelliteId, (sgp4::Elements, DateTime<Utc>)>,
    /// Cached constants for each satellite (avoids expensive reconstruction per-call)
    constants: HashMap<SatelliteId, sgp4::Constants>,
}

impl Propagator {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            constants: HashMap::new(),
        }
    }
}

impl Default for Propagator {
    fn default() -> Self {
        Self::new()
    }
}

impl Propagator {
    /// Load TLE into the propagator
    pub fn load_tle(&mut self, tle: &TleData) -> Result<()> {
        let elements = sgp4::Elements::from_tle(None, tle.line1.as_bytes(), tle.line2.as_bytes())
            .map_err(|e| {
            ground_core::GroundStationError::Tracking(format!("SGP4 parse error: {}", e))
        })?;

        // Cache the constants at load time (expensive operation, do once)
        let constants = sgp4::Constants::from_elements(&elements).map_err(|e| {
            ground_core::GroundStationError::Tracking(format!("SGP4 constants error: {}", e))
        })?;

        self.elements
            .insert(tle.satellite_id.clone(), (elements, tle.epoch));
        self.constants.insert(tle.satellite_id.clone(), constants);
        Ok(())
    }

    /// Propagate to a specific time
    pub fn propagate(
        &self,
        satellite_id: &SatelliteId,
        time: DateTime<Utc>,
    ) -> Result<OrbitalState> {
        let (_elements, tle_epoch) = self.elements.get(satellite_id).ok_or_else(|| {
            ground_core::GroundStationError::Tracking(format!(
                "Satellite {} not loaded",
                satellite_id
            ))
        })?;

        // Use cached constants instead of reconstructing per-call
        let constants = self.constants.get(satellite_id).ok_or_else(|| {
            ground_core::GroundStationError::Tracking(format!(
                "Satellite {} constants not cached",
                satellite_id
            ))
        })?;

        // Compute minutes from epoch
        let minutes_since_epoch = (time - tle_epoch).num_seconds() as f64 / 60.0;

        let state = constants
            .propagate(sgp4::MinutesSinceEpoch(minutes_since_epoch))
            .map_err(|e| {
                ground_core::GroundStationError::Tracking(format!("SGP4 propagation error: {}", e))
            })?;

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

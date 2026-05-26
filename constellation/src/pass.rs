//! Ground station pass prediction

use crate::satellite::OrbitalState;
use chrono::{DateTime, Duration, Utc};
use ground_core::{Result, StationId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Ground station location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundStation {
    /// Station identifier
    pub id: StationId,
    /// Station name
    pub name: String,
    /// Latitude in degrees
    pub latitude_deg: f64,
    /// Longitude in degrees
    pub longitude_deg: f64,
    /// Altitude in meters
    pub altitude_m: f64,
    /// Minimum elevation angle for pass (degrees)
    pub min_elevation_deg: f64,
}

impl GroundStation {
    pub fn new(
        id: StationId,
        name: String,
        latitude_deg: f64,
        longitude_deg: f64,
        altitude_m: f64,
        min_elevation_deg: f64,
    ) -> Self {
        Self {
            id,
            name,
            latitude_deg,
            longitude_deg,
            altitude_m,
            min_elevation_deg,
        }
    }

    /// Convert to ECI coordinates at a given time
    fn to_eci(&self, time: DateTime<Utc>) -> [f64; 3] {
        let lat = self.latitude_deg.to_radians();
        let lon = self.longitude_deg.to_radians();
        let alt_km = self.altitude_m / 1000.0;

        // Earth radius in km (WGS84 approximation)
        const RE: f64 = 6378.137;
        const FLATTENING: f64 = 1.0 / 298.257223563;

        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        let radius = RE / (1.0 - FLATTENING * sin_lat * sin_lat).sqrt() + alt_km;

        // Compute GMST in radians
        let jd = time.timestamp() as f64 / 86400.0 + 2440587.5;
        let gmst = 280.46061837 + 360.98564736629 * (jd - 2451545.0);
        let gmst_rad = (gmst % 360.0).to_radians();

        let theta = lon + gmst_rad;
        let x = radius * cos_lat * theta.cos();
        let y = radius * cos_lat * theta.sin();
        let z = radius * sin_lat;

        [x, y, z]
    }

    /// Compute elevation angle from satellite position
    fn elevation_from_position(&self, satellite_position: [f64; 3], time: DateTime<Utc>) -> f64 {
        let station_eci = self.to_eci(time);
        let rel_pos = [
            satellite_position[0] - station_eci[0],
            satellite_position[1] - station_eci[1],
            satellite_position[2] - station_eci[2],
        ];

        // Compute local ENU (East-North-Up) coordinates
        let lat = self.latitude_deg.to_radians();
        let lon = self.longitude_deg.to_radians();
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        let sin_lon = lon.sin();
        let cos_lon = lon.cos();

        // ECI to ENU rotation
        let east = -sin_lon * rel_pos[0] + cos_lon * rel_pos[1];
        let north =
            -sin_lat * cos_lon * rel_pos[0] - sin_lat * sin_lon * rel_pos[1] + cos_lat * rel_pos[2];
        let up =
            cos_lat * cos_lon * rel_pos[0] + cos_lat * sin_lon * rel_pos[1] + sin_lat * rel_pos[2];

        // Elevation angle
        let range = (east * east + north * north + up * up).sqrt();
        (up / range).asin().to_degrees()
    }
}

/// Pass window for a satellite over a ground station
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassWindow {
    /// When the pass starts (satellite rises above min elevation)
    pub rise_time: DateTime<Utc>,
    /// When the pass ends (satellite sets below min elevation)
    pub set_time: DateTime<Utc>,
    /// Maximum elevation angle during the pass
    pub max_elevation_deg: f64,
    /// Time of maximum elevation
    pub max_elevation_time: DateTime<Utc>,
}

/// A predicted pass with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pass {
    /// Satellite ID
    pub satellite_id: String,
    /// Ground station ID
    pub station_id: StationId,
    /// Pass window
    pub window: PassWindow,
    /// Maximum range during pass (km)
    pub max_range_km: f64,
}

/// Pass predictor for satellite-ground station visibility
pub struct PassPredictor {
    /// Ground stations
    stations: HashMap<StationId, GroundStation>,
}

impl PassPredictor {
    pub fn new() -> Self {
        Self {
            stations: HashMap::new(),
        }
    }

    /// Add a ground station
    pub fn add_station(&mut self, station: GroundStation) {
        self.stations.insert(station.id.clone(), station);
    }

    /// Get a ground station
    pub fn get_station(&self, station_id: &StationId) -> Option<&GroundStation> {
        self.stations.get(station_id)
    }

    /// Predict passes for a satellite over a time window
    pub fn predict_passes(
        &self,
        satellite_id: &str,
        orbital_states: &[OrbitalState],
        station_id: &StationId,
    ) -> Result<Vec<Pass>> {
        let station = self.get_station(station_id).ok_or_else(|| {
            ground_core::GroundStationError::Tracking(format!("Station {} not found", station_id))
        })?;

        let mut passes = Vec::new();
        let mut in_pass = false;
        let mut current_pass: Option<PassWindow> = None;
        let mut max_elevation = 0.0;
        let mut max_elevation_time = None;
        let mut max_range = 0.0;

        for state in orbital_states {
            let elevation = station.elevation_from_position(state.position, state.time);
            let range =
                (state.position[0].powi(2) + state.position[1].powi(2) + state.position[2].powi(2))
                    .sqrt();

            if elevation >= station.min_elevation_deg {
                if !in_pass {
                    // Pass starts
                    in_pass = true;
                    current_pass = Some(PassWindow {
                        rise_time: state.time,
                        set_time: state.time, // Will be updated later
                        max_elevation_deg: 0.0,
                        max_elevation_time: state.time,
                    });
                    max_elevation = elevation;
                    max_elevation_time = Some(state.time);
                    max_range = range;
                } else {
                    // Continue pass
                    if elevation > max_elevation {
                        max_elevation = elevation;
                        max_elevation_time = Some(state.time);
                    }
                    if range > max_range {
                        max_range = range;
                    }
                }
            } else {
                if in_pass {
                    // Pass ends
                    in_pass = false;
                    if let Some(mut window) = current_pass.take() {
                        window.set_time = state.time;
                        window.max_elevation_deg = max_elevation;
                        window.max_elevation_time = max_elevation_time.unwrap_or(state.time);

                        passes.push(Pass {
                            satellite_id: satellite_id.to_string(),
                            station_id: station_id.clone(),
                            window,
                            max_range_km: max_range,
                        });
                    }
                }
            }
        }

        // Handle case where pass ends at the end of the time window
        if in_pass && let Some(mut window) = current_pass.take() {
            window.set_time = orbital_states
                .last()
                .map(|s| s.time)
                .unwrap_or_else(Utc::now);
            window.max_elevation_deg = max_elevation;
            window.max_elevation_time = max_elevation_time.unwrap_or_else(Utc::now);

            passes.push(Pass {
                satellite_id: satellite_id.to_string(),
                station_id: station_id.clone(),
                window,
                max_range_km: max_range,
            });
        }

        Ok(passes)
    }

    /// Predict passes for all satellites over a time window
    pub fn predict_all_passes(
        &self,
        constellation: &crate::manager::Constellation,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        step_sec: u64,
    ) -> Result<HashMap<StationId, Vec<Pass>>> {
        let mut all_passes = HashMap::new();

        for (satellite_id, satellite) in &constellation.satellites {
            // Generate orbital states at regular intervals
            let mut orbital_states = Vec::new();
            let mut current_time = start_time;

            while current_time <= end_time {
                let state = satellite.propagate(current_time)?;
                orbital_states.push(state);
                current_time += Duration::seconds(step_sec as i64);
            }

            // Predict passes for each station
            for station_id in self.stations.keys() {
                let passes = self.predict_passes(satellite_id, &orbital_states, station_id)?;
                if !passes.is_empty() {
                    all_passes
                        .entry(station_id.clone())
                        .or_insert_with(Vec::new)
                        .extend(passes);
                }
            }
        }

        Ok(all_passes)
    }
}

impl Default for PassPredictor {
    fn default() -> Self {
        Self::new()
    }
}

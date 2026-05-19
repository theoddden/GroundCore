// Constellation State Machine - maintains live state model with bi-temporal versioning

use crate::{AssetId, BiTemporal, GeoRegion, SatelliteId, TimeWindow};
use bitemporal::timestamp::{EventTime, ReceptionTime};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Constellation state - live state of the entire constellation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationState {
    pub satellites: HashMap<SatelliteId, SatelliteState>,
    pub optical_terminals: HashMap<crate::TerminalId, TerminalState>,
    pub ground_stations: HashMap<AssetId, GroundStationState>,
    pub current_version: BiTemporal<DateTime<Utc>>,
}

/// Satellite state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteState {
    pub satellite_id: SatelliteId,
    pub orbital_position: OrbitalPosition,
    pub optical_terminals: Vec<crate::TerminalId>,
    pub rf_terminals: Vec<crate::TerminalId>,
    pub health: SatelliteHealth,
    pub current_missions: Vec<crate::TaskId>,
    pub last_updated: BiTemporal<DateTime<Utc>>,
}

/// Orbital position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitalPosition {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_km: f64,
    pub velocity_kms: f64,
    pub tle: Option<String>, // Two-line element set
}

/// Terminal state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalState {
    pub terminal_id: crate::TerminalId,
    pub satellite_id: SatelliteId,
    pub vendor: crate::physical::Vendor,
    pub model: String,
    pub current_link: Option<crate::LinkId>,
    pub available: bool,
    pub health: TerminalHealth,
    pub last_updated: BiTemporal<DateTime<Utc>>,
}

/// Satellite health
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SatelliteHealth {
    Nominal,
    Degraded,
    Critical,
}

/// Terminal health
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalHealth {
    Nominal,
    Degraded,
    Offline,
    Failed,
}

/// Ground station state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundStationState {
    pub station_id: AssetId,
    pub location: (f64, f64), // latitude, longitude
    pub optical_terminals: Vec<crate::TerminalId>,
    pub rf_terminals: Vec<crate::TerminalId>,
    pub availability: Vec<TimeWindow>,
    pub weather_conditions: WeatherConditions,
}

/// Weather conditions for ground station
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConditions {
    pub cloud_cover_percent: f64,
    pub visibility_km: f64,
    pub wind_speed_kmh: f64,
    pub precipitation_mm: Option<f64>,
}

impl ConstellationState {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            satellites: HashMap::new(),
            optical_terminals: HashMap::new(),
            ground_stations: HashMap::new(),
            current_version: BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now)),
        }
    }

    /// Resolve an asset ID to a satellite ID
    pub fn resolve_asset_to_satellite(&self, asset: &AssetId) -> Option<SatelliteId> {
        // Check if asset is a satellite
        if self.satellites.contains_key(asset) {
            return Some(asset.clone());
        }

        // Check if asset is associated with a satellite via terminal
        for terminal in self.optical_terminals.values() {
            if terminal.terminal_id.to_string() == *asset {
                return Some(terminal.satellite_id.clone());
            }
        }

        None
    }

    /// Find optimal ground station for downlink
    pub fn find_optimal_ground_station(
        &self,
        satellite: &SatelliteId,
        window: &TimeWindow,
        _constraints: &crate::mission::IntentConstraints,
    ) -> Option<AssetId> {
        let sat_state = self.satellites.get(satellite)?;

        let mut best_station = None;
        let mut best_score = 0.0;

        for (station_id, station) in &self.ground_stations {
            // Check if station is available during window
            let available = station
                .availability
                .iter()
                .any(|avail| avail.overlaps(window));

            if !available {
                continue;
            }

            // Check weather conditions
            if station.weather_conditions.cloud_cover_percent > 80.0 {
                continue; // Too cloudy for optical
            }

            // Calculate geometry score
            let score = self.calculate_geometry_score(
                &sat_state.orbital_position,
                &station.location,
                &station.weather_conditions,
            );

            if score > best_score {
                best_score = score;
                best_station = Some(station_id.clone());
            }
        }

        best_station
    }

    fn calculate_geometry_score(
        &self,
        orb_pos: &OrbitalPosition,
        station_loc: &(f64, f64),
        weather: &WeatherConditions,
    ) -> f64 {
        // Simple elevation angle calculation
        let lat_diff = (orb_pos.latitude - station_loc.0).to_radians();
        let lon_diff = (orb_pos.longitude - station_loc.1).to_radians();
        let elevation = (lat_diff.sin() * lat_diff.sin()
            + lat_diff.cos() * lat_diff.cos() * lon_diff.cos())
        .acos();

        let elevation_deg = elevation.to_degrees();

        // Higher elevation = better
        let elevation_score = if elevation_deg > 30.0 {
            1.0
        } else if elevation_deg > 10.0 {
            0.7
        } else {
            0.3
        };

        // Better weather = better
        let weather_score = 1.0 - (weather.cloud_cover_percent / 100.0);

        (elevation_score + weather_score) / 2.0
    }

    /// Update satellite state with bi-temporal versioning
    pub fn update_satellite(&mut self, satellite_id: &SatelliteId, mut state: SatelliteState) {
        let now = Utc::now();
        state.last_updated = BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now));
        self.satellites.insert(satellite_id.clone(), state);
        self.current_version = BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now));
    }

    /// Update terminal state with bi-temporal versioning
    pub fn update_terminal(&mut self, terminal_id: &crate::TerminalId, mut state: TerminalState) {
        let now = Utc::now();
        state.last_updated = BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now));
        self.optical_terminals.insert(terminal_id.clone(), state);
        self.current_version = BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now));
    }
}

impl Default for ConstellationState {
    fn default() -> Self {
        Self::new()
    }
}

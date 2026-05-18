//! Individual satellite wrapper around SGP4

use chrono::{DateTime, Utc};
use ground_core::{SatelliteId, Result};
use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements, MinutesSinceEpoch};

/// Individual satellite with SGP4 propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Satellite {
    /// Satellite identifier
    pub id: SatelliteId,
    /// Satellite name (e.g., "STARLINK-1234")
    pub name: String,
    /// NORAD catalog number
    pub norad_id: u32,
    /// SGP4 orbital elements (serializable for persistence)
    pub elements: Elements,
    /// TLE epoch (from elements.datetime, duplicated for convenience)
    pub tle_epoch: DateTime<Utc>,
    /// SGP4 constants (computed from elements, not serialized)
    #[serde(skip)]
    pub constants: Option<Constants>,
    /// Source of this TLE (e.g., "Celestrak", "Space-Track")
    pub source: String,
}

impl Satellite {
    /// Create a new satellite from TLE data
    pub fn from_tle(
        id: SatelliteId,
        name: String,
        norad_id: u32,
        line1: &str,
        line2: &str,
        source: String,
    ) -> Result<Self> {
        let elements = Elements::from_tle(Some(name.clone()), line1.as_bytes(), line2.as_bytes())
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 TLE parse error: {}", e)))?;

        let tle_epoch = elements.datetime.and_utc();

        Ok(Self {
            id,
            name,
            norad_id,
            elements,
            tle_epoch,
            constants: None,
            source,
        })
    }

    /// Initialize SGP4 constants from elements
    pub fn initialize_constants(&mut self) -> Result<()> {
        let constants = Constants::from_elements(&self.elements)
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 constants error: {}", e)))?;
        self.constants = Some(constants);
        Ok(())
    }

    /// Propagate to a specific time
    pub fn propagate(&self, time: DateTime<Utc>) -> Result<OrbitalState> {
        let constants = self.constants.as_ref()
            .ok_or_else(|| ground_core::GroundStationError::Tracking("SGP4 constants not initialized".to_string()))?;

        let minutes_since_epoch = (time - self.tle_epoch).num_seconds() as f64 / 60.0;
        let prediction = constants.propagate(MinutesSinceEpoch(minutes_since_epoch))
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 propagation error: {}", e)))?;

        Ok(OrbitalState {
            position: [prediction.position[0], prediction.position[1], prediction.position[2]],
            velocity: [prediction.velocity[0], prediction.velocity[1], prediction.velocity[2]],
            time,
            tle_epoch: self.tle_epoch,
        })
    }

    /// Check if TLE needs refresh (older than threshold hours)
    pub fn needs_refresh(&self, max_age_hours: i64) -> bool {
        let age = Utc::now() - self.tle_epoch;
        age.num_hours() > max_age_hours
    }

    /// Refresh TLE from new data
    pub fn refresh_tle(&mut self, line1: &str, line2: &str, source: String) -> Result<()> {
        let elements = Elements::from_tle(Some(self.name.clone()), line1.as_bytes(), line2.as_bytes())
            .map_err(|e| ground_core::GroundStationError::Tracking(format!("SGP4 TLE parse error: {}", e)))?;

        self.elements = elements;
        self.tle_epoch = self.elements.datetime.and_utc();
        self.source = source;
        self.constants = None; // Re-initialize on next use
        Ok(())
    }
}

/// Orbital state at a specific time (compatible with tracking::OrbitalState)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitalState {
    /// Position in ECI coordinates (km)
    pub position: [f64; 3],
    /// Velocity in ECI coordinates (km/s)
    pub velocity: [f64; 3],
    /// When this state is valid
    pub time: DateTime<Utc>,
    /// TLE epoch used for propagation
    pub tle_epoch: DateTime<Utc>,
}

impl OrbitalState {
    /// Convert to tracking crate's OrbitalState
    pub fn to_tracking_state(&self) -> tracking::OrbitalState {
        tracking::OrbitalState {
            position: nalgebra::Vector3::new(self.position[0], self.position[1], self.position[2]),
            velocity: nalgebra::Vector3::new(self.velocity[0], self.velocity[1], self.velocity[2]),
            time: self.time,
            tle_epoch: self.tle_epoch,
        }
    }
}

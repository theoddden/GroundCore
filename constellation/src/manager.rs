//! Constellation manager for multi-satellite orbital state

use crate::satellite::{OrbitalState, Satellite};
use chrono::{DateTime, Utc};
use ground_core::{Result, SatelliteId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manager for multiple constellations
#[derive(Debug)]
pub struct ConstellationManager {
    /// Constellations indexed by name (e.g., "starlink", "oneweb")
    constellations: HashMap<String, Constellation>,
}

/// A single constellation (group of satellites)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constellation {
    /// Constellation name
    pub name: String,
    /// Satellites in this constellation
    pub satellites: HashMap<SatelliteId, Satellite>,
    /// When this constellation was last updated
    pub updated_at: DateTime<Utc>,
}

/// Snapshot of constellation state at a specific time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationSnapshot {
    /// Constellation name
    pub constellation_name: String,
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Orbital states for all satellites
    pub states: HashMap<SatelliteId, OrbitalState>,
}

impl Constellation {
    pub fn new(name: String) -> Self {
        Self {
            name,
            satellites: HashMap::new(),
            updated_at: Utc::now(),
        }
    }

    /// Add a satellite to the constellation
    pub fn add_satellite(&mut self, satellite: Satellite) {
        self.satellites.insert(satellite.id.clone(), satellite);
        self.updated_at = Utc::now();
    }

    /// Remove a satellite
    pub fn remove_satellite(&mut self, satellite_id: &SatelliteId) -> Option<Satellite> {
        self.satellites.remove(satellite_id)
    }

    /// Get a satellite
    pub fn get_satellite(&self, satellite_id: &SatelliteId) -> Option<&Satellite> {
        self.satellites.get(satellite_id)
    }

    /// Get all satellite IDs
    pub fn satellite_ids(&self) -> Vec<SatelliteId> {
        self.satellites.keys().cloned().collect()
    }

    /// Initialize SGP4 constants for all satellites
    pub fn initialize_all(&mut self) -> Result<()> {
        for satellite in self.satellites.values_mut() {
            satellite.initialize_constants()?;
        }
        Ok(())
    }

    /// Propagate entire constellation to a specific time
    pub fn propagate(&self, time: DateTime<Utc>) -> Result<ConstellationSnapshot> {
        let mut states = HashMap::new();

        for (id, satellite) in &self.satellites {
            let state = satellite.propagate(time)?;
            states.insert(id.clone(), state);
        }

        Ok(ConstellationSnapshot {
            constellation_name: self.name.clone(),
            timestamp: time,
            states,
        })
    }

    /// Batch refresh TLEs for satellites that need it
    pub fn batch_refresh<F>(&mut self, fetch_fn: F, max_age_hours: i64) -> Result<()>
    where
        F: Fn(&SatelliteId) -> Result<(String, String, String)>,
    {
        let satellites_to_refresh: Vec<SatelliteId> = self
            .satellites
            .iter()
            .filter(|(_, sat)| sat.needs_refresh(max_age_hours))
            .map(|(id, _)| id.clone())
            .collect();

        for satellite_id in satellites_to_refresh {
            if let Some(satellite) = self.satellites.get_mut(&satellite_id) {
                match fetch_fn(&satellite_id) {
                    Ok((line1, line2, source)) => {
                        if let Err(e) = satellite.refresh_tle(&line1, &line2, source) {
                            tracing::warn!("Failed to refresh TLE for {}: {}", satellite_id, e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch TLE for {}: {}", satellite_id, e);
                    }
                }
            }
        }

        self.updated_at = Utc::now();
        Ok(())
    }
}

impl ConstellationManager {
    pub fn new() -> Self {
        Self {
            constellations: HashMap::new(),
        }
    }

    /// Add a constellation
    pub fn add_constellation(&mut self, constellation: Constellation) {
        self.constellations
            .insert(constellation.name.clone(), constellation);
    }

    /// Get a constellation
    pub fn get_constellation(&self, name: &str) -> Option<&Constellation> {
        self.constellations.get(name)
    }

    /// Get mutable reference to a constellation
    pub fn get_constellation_mut(&mut self, name: &str) -> Option<&mut Constellation> {
        self.constellations.get_mut(name)
    }

    /// Get all constellation names
    pub fn constellation_names(&self) -> Vec<String> {
        self.constellations.keys().cloned().collect()
    }

    /// Initialize all constellations
    pub fn initialize_all(&mut self) -> Result<()> {
        for constellation in self.constellations.values_mut() {
            constellation.initialize_all()?;
        }
        Ok(())
    }

    /// Propagate all constellations to a specific time
    pub fn propagate_all(
        &self,
        time: DateTime<Utc>,
    ) -> Result<HashMap<String, ConstellationSnapshot>> {
        let mut snapshots = HashMap::new();

        for (name, constellation) in &self.constellations {
            let snapshot = constellation.propagate(time)?;
            snapshots.insert(name.clone(), snapshot);
        }

        Ok(snapshots)
    }

    /// Get a specific satellite from any constellation
    pub fn get_satellite(&self, satellite_id: &SatelliteId) -> Option<&Satellite> {
        for constellation in self.constellations.values() {
            if let Some(satellite) = constellation.get_satellite(satellite_id) {
                return Some(satellite);
            }
        }
        None
    }

    /// Propagate a specific satellite
    pub fn propagate_satellite(
        &self,
        satellite_id: &SatelliteId,
        time: DateTime<Utc>,
    ) -> Result<OrbitalState> {
        let satellite = self.get_satellite(satellite_id).ok_or_else(|| {
            ground_core::GroundStationError::Tracking(format!(
                "Satellite {} not found",
                satellite_id
            ))
        })?;
        satellite.propagate(time)
    }
}

impl Default for ConstellationManager {
    fn default() -> Self {
        Self::new()
    }
}

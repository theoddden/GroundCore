// Mutual line-of-sight prediction
//
// Visibility prediction must account for solar exclusion - if the sun is too close
// to the line-of-sight vector, the optical sensor saturates and link is impossible.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Visibility window
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibilityWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub max_elevation_deg: f64,
    pub min_range_km: f64,
    pub max_range_km: f64,
}

impl VisibilityWindow {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            start,
            end,
            max_elevation_deg: 0.0,
            min_range_km: 0.0,
            max_range_km: 0.0,
        }
    }

    pub fn duration(&self) -> Duration {
        self.end - self.start
    }

    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp >= self.start && timestamp <= self.end
    }
}

/// Visibility constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityConstraints {
    pub min_elevation_deg: f64,
    pub max_range_km: f64,
    pub forbidden_zones: Vec<ForbiddenZone>,
    pub solar_exclusion_angle_deg: f64,
}

impl Default for VisibilityConstraints {
    fn default() -> Self {
        Self {
            min_elevation_deg: 5.0,
            max_range_km: 5000.0,
            forbidden_zones: Vec::new(),
            solar_exclusion_angle_deg: 10.0,
        }
    }
}

impl VisibilityConstraints {
    pub fn with_solar_exclusion(angle_deg: f64) -> Self {
        let mut constraints = Self::default();
        constraints.solar_exclusion_angle_deg = angle_deg;
        constraints
    }
}

/// Forbidden zone (e.g., sun-blinding angles)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForbiddenZone {
    pub center_azimuth_rad: f64,
    pub center_elevation_rad: f64,
    pub radius_rad: f64,
    pub reason: String,
}

impl ForbiddenZone {
    pub fn solar_exclusion(
        sun_azimuth_rad: f64,
        sun_elevation_rad: f64,
        exclusion_angle_deg: f64,
    ) -> Self {
        Self {
            center_azimuth_rad: sun_azimuth_rad,
            center_elevation_rad: sun_elevation_rad,
            radius_rad: exclusion_angle_deg.to_radians(),
            reason: "Solar exclusion".to_string(),
        }
    }

    pub fn contains(&self, azimuth_rad: f64, elevation_rad: f64) -> bool {
        // Simple angular distance check
        let az_diff = (azimuth_rad - self.center_azimuth_rad).abs();
        let el_diff = (elevation_rad - self.center_elevation_rad).abs();
        let distance = (az_diff.powi(2) + el_diff.powi(2)).sqrt();
        distance < self.radius_rad
    }
}

/// Predict visibility windows between two satellites
pub fn predict_visibility_window(
    _observer_orbit: &Orbit,
    _target_orbit: &Orbit,
    _constraints: &VisibilityConstraints,
    _forecast_horizon: Duration,
) -> Vec<VisibilityWindow> {
    // Simplified implementation - in production this would use SGP4 propagation
    let now = Utc::now();

    // Generate sample windows (placeholder)
    vec![
        VisibilityWindow {
            start: now + Duration::minutes(10),
            end: now + Duration::minutes(20),
            max_elevation_deg: 45.0,
            min_range_km: 1000.0,
            max_range_km: 3000.0,
        },
        VisibilityWindow {
            start: now + Duration::minutes(90),
            end: now + Duration::minutes(105),
            max_elevation_deg: 60.0,
            min_range_km: 800.0,
            max_range_km: 2500.0,
        },
    ]
}

/// Orbit placeholder
#[derive(Debug, Clone)]
pub struct Orbit {
    pub satellite_id: String,
    pub tle: String,
}

impl Orbit {
    pub fn new(satellite_id: String, tle: String) -> Self {
        Self { satellite_id, tle }
    }
}

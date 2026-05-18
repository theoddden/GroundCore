//! Homing procedures and safety stow

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Stow position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StowPosition {
    /// Azimuth in degrees
    pub azimuth: f64,
    /// Elevation in degrees
    pub elevation: f64,
    /// Stow position name
    pub name: String,
}

impl StowPosition {
    /// Create a new stow position
    pub fn new(azimuth: f64, elevation: f64, name: &str) -> Self {
        Self {
            azimuth,
            elevation,
            name: name.to_string(),
        }
    }

    /// Zenith stow (pointing straight up)
    pub fn zenith() -> Self {
        Self::new(0.0, 90.0, "zenith")
    }

    /// Horizon stow (pointing at horizon)
    pub fn horizon() -> Self {
        Self::new(0.0, 0.0, "horizon")
    }
}

/// Calibration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationResult {
    /// Calibration successful
    pub success: bool,
    /// Azimuth offset (degrees)
    pub azimuth_offset: f64,
    /// Elevation offset (degrees)
    pub elevation_offset: f64,
    /// Calibration timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Homing procedure
pub struct HomingProcedure {
    /// Index mark azimuth position
    index_azimuth: f64,
    /// Index mark elevation position
    index_elevation: f64,
    /// Current calibration offsets
    azimuth_offset: f64,
    elevation_offset: f64,
    /// Calibrated flag
    calibrated: bool,
}

impl HomingProcedure {
    /// Create a new homing procedure
    pub fn new(index_azimuth: f64, index_elevation: f64) -> Self {
        Self {
            index_azimuth,
            index_elevation,
            azimuth_offset: 0.0,
            elevation_offset: 0.0,
            calibrated: false,
        }
    }

    /// Find index marks (home antenna)
    pub async fn find_index_marks(&mut self) -> CalibrationResult {
        // In a real implementation, this would:
        // 1. Move antenna to mechanical stops
        // 2. Read encoder values at index marks
        // 3. Store offsets
        
        // Simulate homing
        self.azimuth_offset = 0.0;
        self.elevation_offset = 0.0;
        self.calibrated = true;

        CalibrationResult {
            success: true,
            azimuth_offset: self.azimuth_offset,
            elevation_offset: self.elevation_offset,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Calibrate antenna to known reference
    pub async fn calibrate_to_reference(
        &mut self,
        reference_az: f64,
        reference_el: f64,
    ) -> CalibrationResult {
        // In a real implementation, this would:
        // 1. Point antenna at known reference (e.g., celestial source)
        // 2. Compare commanded vs. actual position
        // 3. Store calibration offsets

        self.azimuth_offset = reference_az - self.index_azimuth;
        self.elevation_offset = reference_el - self.index_elevation;
        self.calibrated = true;

        CalibrationResult {
            success: true,
            azimuth_offset: self.azimuth_offset,
            elevation_offset: self.elevation_offset,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Get current calibration offsets
    pub fn get_offsets(&self) -> (f64, f64) {
        (self.azimuth_offset, self.elevation_offset)
    }

    /// Check if antenna is calibrated
    pub fn is_calibrated(&self) -> bool {
        self.calibrated
    }

    /// Reset calibration
    pub fn reset(&mut self) {
        self.azimuth_offset = 0.0;
        self.elevation_offset = 0.0;
        self.calibrated = false;
    }

    /// Apply calibration offsets to position
    pub fn apply_calibration(&self, azimuth: f64, elevation: f64) -> (f64, f64) {
        let calibrated_az = azimuth + self.azimuth_offset;
        let calibrated_el = elevation + self.elevation_offset;
        (calibrated_az, calibrated_el)
    }
}

/// Sun avoidance
pub struct SunAvoidance {
    /// Sun position threshold (degrees from sun to avoid)
    sun_threshold: f64,
    /// Current sun position (az, el)
    sun_position: Option<(f64, f64)>,
}

impl SunAvoidance {
    /// Create a new sun avoidance system
    pub fn new(sun_threshold: f64) -> Self {
        Self {
            sun_threshold,
            sun_position: None,
        }
    }

    /// Update sun position
    pub fn update_sun_position(&mut self, azimuth: f64, elevation: f64) {
        self.sun_position = Some((azimuth, elevation));
    }

    /// Check if pointing direction is too close to sun
    pub fn is_sun_conflict(&self, azimuth: f64, elevation: f64) -> bool {
        if let Some((sun_az, sun_el)) = self.sun_position {
            let angular_separation = self.angular_separation(azimuth, elevation, sun_az, sun_el);
            angular_separation < self.sun_threshold
        } else {
            false
        }
    }

    /// Compute angular separation between two directions
    fn angular_separation(&self, az1: f64, el1: f64, az2: f64, el2: f64) -> f64 {
        // Convert to Cartesian coordinates
        let x1 = el1.to_radians().cos() * az1.to_radians().cos();
        let y1 = el1.to_radians().cos() * az1.to_radians().sin();
        let z1 = el1.to_radians().sin();

        let x2 = el2.to_radians().cos() * az2.to_radians().cos();
        let y2 = el2.to_radians().cos() * az2.to_radians().sin();
        let z2 = el2.to_radians().sin();

        // Dot product
        let dot = x1 * x2 + y1 * y2 + z1 * z2;
        
        // Clamp to [-1, 1] to handle floating point errors
        let dot = dot.clamp(-1.0, 1.0);
        
        // Angular separation
        dot.acos().to_degrees()
    }

    /// Get sun-safe pointing direction (away from sun)
    pub fn get_safe_pointing(&self, target_az: f64, target_el: f64) -> Option<(f64, f64)> {
        if !self.is_sun_conflict(target_az, target_el) {
            return Some((target_az, target_el));
        }

        if let Some((sun_az, sun_el)) = self.sun_position {
            // Point opposite to sun
            let safe_az = (sun_az + 180.0) % 360.0;
            let safe_el = 90.0 - sun_el; // Mirror elevation
            Some((safe_az, safe_el))
        } else {
            None
        }
    }
}

// Acquisition search patterns
//
// When two terminals begin acquisition, neither knows exactly where the other is
// in their respective fields of view. They each execute a search pattern centered
// on the predicted pointing vector, scanning outward until they detect a beacon.

use serde::{Deserialize, Serialize};

/// Search pattern type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchPattern {
    Spiral {
        initial_radius_urad: f64,
        spiral_pitch_urad: f64,
        max_radius_urad: f64,
        scan_rate_urad_per_sec: f64,
    },
    Raster {
        width_urad: f64,
        height_urad: f64,
        line_spacing_urad: f64,
    },
    Lissajous {
        amplitude_x_urad: f64,
        amplitude_y_urad: f64,
        frequency_ratio: f64,
    },
}

impl SearchPattern {
    pub fn spiral_default() -> Self {
        Self::Spiral {
            initial_radius_urad: 10.0,
            spiral_pitch_urad: 5.0,
            max_radius_urad: 500.0,
            scan_rate_urad_per_sec: 100.0,
        }
    }

    pub fn raster_default() -> Self {
        Self::Raster {
            width_urad: 1000.0,
            height_urad: 1000.0,
            line_spacing_urad: 50.0,
        }
    }

    pub fn lissajous_default() -> Self {
        Self::Lissajous {
            amplitude_x_urad: 500.0,
            amplitude_y_urad: 500.0,
            frequency_ratio: 3.0,
        }
    }

    pub fn estimated_duration_ms(&self) -> u64 {
        match self {
            Self::Spiral {
                max_radius_urad,
                scan_rate_urad_per_sec,
                ..
            } => {
                let area = std::f64::consts::PI * max_radius_urad.powi(2);
                let scan_area_per_ms = scan_rate_urad_per_sec / 1000.0;
                (area / scan_area_per_ms) as u64
            }
            Self::Raster {
                width_urad,
                height_urad,
                line_spacing_urad,
                ..
            } => {
                let total_scan_urad = (width_urad * height_urad) / line_spacing_urad;
                (total_scan_urad / 50.0) as u64 // Assume 50 urad/ms scan rate
            }
            Self::Lissajous {
                amplitude_x_urad,
                amplitude_y_urad,
                ..
            } => {
                let perimeter = 2.0 * (amplitude_x_urad + amplitude_y_urad);
                (perimeter / 50.0) as u64
            }
        }
    }
}

/// Search state during acquisition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchState {
    pub pattern: SearchPattern,
    pub current_angle_x_urad: f64,
    pub current_angle_y_urad: f64,
    pub steps_completed: u32,
    pub beacon_detected: bool,
    pub detection_confidence: f64,
}

impl SearchState {
    pub fn new(pattern: SearchPattern) -> Self {
        Self {
            pattern,
            current_angle_x_urad: 0.0,
            current_angle_y_urad: 0.0,
            steps_completed: 0,
            beacon_detected: false,
            detection_confidence: 0.0,
        }
    }
}

/// Spiral search pattern
#[derive(Debug, Clone)]
pub struct SpiralPattern {
    initial_radius_urad: f64,
    spiral_pitch_urad: f64,
    #[allow(dead_code)]
    max_radius_urad: f64,
    #[allow(dead_code)]
    scan_rate_urad_per_sec: f64,
}

impl SpiralPattern {
    pub fn new(
        initial_radius_urad: f64,
        spiral_pitch_urad: f64,
        max_radius_urad: f64,
        scan_rate_urad_per_sec: f64,
    ) -> Self {
        Self {
            initial_radius_urad,
            spiral_pitch_urad,
            max_radius_urad,
            scan_rate_urad_per_sec,
        }
    }

    pub fn position_at_step(&self, step: u32) -> (f64, f64) {
        let radius = self.initial_radius_urad + (step as f64) * self.spiral_pitch_urad;
        let angle = (step as f64) * 0.5; // Golden angle approximation
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        (x, y)
    }
}

/// Raster search pattern
#[derive(Debug, Clone)]
pub struct RasterPattern {
    width_urad: f64,
    height_urad: f64,
    line_spacing_urad: f64,
}

impl RasterPattern {
    pub fn new(width_urad: f64, height_urad: f64, line_spacing_urad: f64) -> Self {
        Self {
            width_urad,
            height_urad,
            line_spacing_urad,
        }
    }

    pub fn position_at_step(&self, step: u32) -> (f64, f64) {
        let lines = (self.height_urad / self.line_spacing_urad) as u32;
        let line = step % lines;
        let direction = if (step / lines).is_multiple_of(2) { 1.0 } else { -1.0 };

        let x = (step as f64 % (self.width_urad / self.line_spacing_urad))
            * self.line_spacing_urad
            * direction;
        let y = (line as f64) * self.line_spacing_urad - (self.height_urad / 2.0);

        (x, y)
    }
}

/// Lissajous search pattern
#[derive(Debug, Clone)]
pub struct LissajousPattern {
    amplitude_x_urad: f64,
    amplitude_y_urad: f64,
    frequency_ratio: f64,
}

impl LissajousPattern {
    pub fn new(amplitude_x_urad: f64, amplitude_y_urad: f64, frequency_ratio: f64) -> Self {
        Self {
            amplitude_x_urad,
            amplitude_y_urad,
            frequency_ratio,
        }
    }

    pub fn position_at_time_ms(&self, time_ms: f64) -> (f64, f64) {
        let t = time_ms / 1000.0;
        let x = self.amplitude_x_urad * (2.0 * std::f64::consts::PI * t).cos();
        let y =
            self.amplitude_y_urad * (2.0 * std::f64::consts::PI * self.frequency_ratio * t).sin();
        (x, y)
    }
}

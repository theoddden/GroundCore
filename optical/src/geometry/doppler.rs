// Optical Doppler prediction
//
// Doppler shift matters for optical links because the wavelength is so small.
// A relative velocity of 7 km/s produces a significant frequency shift.

use serde::{Deserialize, Serialize};
use crate::geometry::pointing::{SatellitePosition, compute_range_rate};

/// Doppler prediction
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DopplerPrediction {
    pub doppler_shift_hz: f64,
    pub wavelength_nm: f64,
    pub relative_velocity_km_s: f64,
}

impl DopplerPrediction {
    pub fn from_relative_velocity(relative_velocity_km_s: f64, wavelength_nm: f64) -> Self {
        // Doppler shift formula: f' = f * (c + v) / c
        // For optical: Δf = f * v / c
        // f = c / λ, so Δf = v / λ
        let _c_km_s = 299_792.458; // Speed of light in km/s
        let wavelength_km = wavelength_nm * 1e-12; // Convert nm to km
        let doppler_shift_hz = relative_velocity_km_s / wavelength_km;
        
        Self {
            doppler_shift_hz,
            wavelength_nm,
            relative_velocity_km_s,
        }
    }

    pub fn compensation_factor(&self) -> f64 {
        // Factor to apply to receiver frequency to compensate
        1.0 + (self.relative_velocity_km_s / 299_792.458)
    }
}

/// Compute optical Doppler shift
pub fn compute_optical_doppler(
    observer: &SatellitePosition,
    target: &SatellitePosition,
    wavelength_nm: f64,
) -> DopplerPrediction {
    let range_rate_km_s = compute_range_rate(observer, target);
    DopplerPrediction::from_relative_velocity(range_rate_km_s, wavelength_nm)
}

/// Common optical wavelengths
pub mod wavelengths {
    pub const INFRARED_1550_NM: f64 = 1550.0;
    pub const INFRARED_1064_NM: f64 = 1064.0;
    pub const VISIBLE_850_NM: f64 = 850.0;
    pub const VISIBLE_780_NM: f64 = 780.0;
}

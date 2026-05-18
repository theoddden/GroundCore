// Atmospheric path loss
//
// Atmospheric attenuation matters only for space-to-ground optical links.
// Inter-satellite links operate in vacuum.

use serde::{Deserialize, Serialize};

/// Atmospheric path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtmosphericPath {
    pub ground_station_id: String,
    pub elevation_angle_deg: f64,
    pub path_length_km: f64,
    pub altitude_profile_km: Vec<f64>,
}

impl AtmosphericPath {
    pub fn new(ground_station_id: String, elevation_angle_deg: f64, path_length_km: f64) -> Self {
        Self {
            ground_station_id,
            elevation_angle_deg,
            path_length_km,
            altitude_profile_km: vec![],
        }
    }
}

/// Weather conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConditions {
    pub cloud_cover_percent: f64,
    pub visibility_km: f64,
    pub humidity_percent: f64,
    pub temperature_c: f64,
    pub pressure_hpa: f64,
    pub wind_speed_kmh: f64,
}

impl WeatherConditions {
    pub fn clear() -> Self {
        Self {
            cloud_cover_percent: 0.0,
            visibility_km: 50.0,
            humidity_percent: 30.0,
            temperature_c: 20.0,
            pressure_hpa: 1013.0,
            wind_speed_kmh: 10.0,
        }
    }

    pub fn moderate() -> Self {
        Self {
            cloud_cover_percent: 30.0,
            visibility_km: 20.0,
            humidity_percent: 50.0,
            temperature_c: 15.0,
            pressure_hpa: 1010.0,
            wind_speed_kmh: 20.0,
        }
    }

    pub fn storm() -> Self {
        Self {
            cloud_cover_percent: 90.0,
            visibility_km: 5.0,
            humidity_percent: 80.0,
            temperature_c: 10.0,
            pressure_hpa: 1000.0,
            wind_speed_kmh: 50.0,
        }
    }

    pub fn attenuation_factor(&self) -> f64 {
        // Simple model: more clouds and humidity = more attenuation
        let cloud_factor = self.cloud_cover_percent / 100.0;
        let humidity_factor = self.humidity_percent / 100.0;
        let visibility_factor = 1.0 - (10.0 / (self.visibility_km + 10.0));
        
        cloud_factor * 0.5 + humidity_factor * 0.3 + visibility_factor * 0.2
    }
}

/// Compute atmospheric attenuation
pub fn compute_atmospheric_attenuation(
    path: &AtmosphericPath,
    wavelength_nm: f64,
    weather: &WeatherConditions,
) -> AttenuationDb {
    // Simplified atmospheric attenuation model
    // In production, this would use MODTRAN or similar
    
    // Base attenuation from atmospheric absorption
    let base_attenuation_db = if wavelength_nm < 1000.0 {
        2.0 // Visible light has more atmospheric scattering
    } else {
        1.0 // Near-infrared has better atmospheric transmission
    };
    
    // Elevation angle correction (lower elevation = more atmosphere)
    let elevation_factor = 1.0 / (path.elevation_angle_deg.to_radians().sin().max(0.1));
    
    // Weather factor
    let weather_factor = weather.attenuation_factor() * 10.0; // Up to 10 dB additional loss
    
    // Total attenuation
    let total_db = base_attenuation_db * elevation_factor + weather_factor;
    
    AttenuationDb(total_db)
}

/// Attenuation in decibels
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AttenuationDb(pub f64);

impl AttenuationDb {
    pub fn new(db: f64) -> Self {
        Self(db)
    }

    pub fn to_linear_ratio(&self) -> f64 {
        10.0_f64.powf(-self.0 / 10.0)
    }

    pub fn is_acceptable(&self) -> bool {
        self.0 < 10.0 // Less than 10 dB attenuation is acceptable
    }

    pub fn is_critical(&self) -> bool {
        self.0 > 20.0 // More than 20 dB is critical
    }
}

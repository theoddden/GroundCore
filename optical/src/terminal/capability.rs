// Terminal capability descriptions

use serde::{Deserialize, Serialize};

/// Vendor identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vendor {
    Mynaric,
    Tesat,
    Skyloom,
    Caci,
}

impl Vendor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mynaric => "Mynaric",
            Self::Tesat => "Tesat",
            Self::Skyloom => "Skyloom",
            Self::Caci => "CACI",
        }
    }
}

/// Terminal capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalCapability {
    pub vendor: Vendor,
    pub model: String,
    pub max_data_rate_bps: u64,
    pub min_elevation_deg: f64,
    pub field_of_regard: FieldOfRegard,
    pub power_profile: PowerProfile,
    pub supported_standards: Vec<String>,
    pub max_range_km: f64,
}

impl TerminalCapability {
    pub fn condor_mk3() -> Self {
        Self {
            vendor: Vendor::Mynaric,
            model: "CONDOR Mk3".to_string(),
            max_data_rate_bps: 10_000_000_000,
            min_elevation_deg: 5.0,
            field_of_regard: FieldOfRegard::new(120.0, 120.0),
            power_profile: PowerProfile {
                operating_watts: 150.0,
                standby_watts: 30.0,
            },
            supported_standards: vec!["OCT 4.0.0".to_string()],
            max_range_km: 5000.0,
        }
    }

    pub fn scot80() -> Self {
        Self {
            vendor: Vendor::Tesat,
            model: "SCOT80".to_string(),
            max_data_rate_bps: 5_000_000_000,
            min_elevation_deg: 10.0,
            field_of_regard: FieldOfRegard::new(90.0, 90.0),
            power_profile: PowerProfile {
                operating_watts: 120.0,
                standby_watts: 25.0,
            },
            supported_standards: vec!["OCT 3.2.0".to_string()],
            max_range_km: 3000.0,
        }
    }

    pub fn scot135() -> Self {
        Self {
            vendor: Vendor::Tesat,
            model: "SCOT135".to_string(),
            max_data_rate_bps: 10_000_000_000,
            min_elevation_deg: 5.0,
            field_of_regard: FieldOfRegard::new(110.0, 110.0),
            power_profile: PowerProfile {
                operating_watts: 180.0,
                standby_watts: 35.0,
            },
            supported_standards: vec!["OCT 4.0.0".to_string()],
            max_range_km: 5000.0,
        }
    }
}

/// Field of regard
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FieldOfRegard {
    pub azimuth_deg: f64,
    pub elevation_deg: f64,
}

impl FieldOfRegard {
    pub fn new(azimuth_deg: f64, elevation_deg: f64) -> Self {
        Self {
            azimuth_deg,
            elevation_deg,
        }
    }

    pub fn total_solid_angle_sr(&self) -> f64 {
        let az_rad = self.azimuth_deg.to_radians();
        let el_rad = self.elevation_deg.to_radians();
        az_rad * el_rad
    }
}

/// Power profile
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerProfile {
    pub operating_watts: f64,
    pub standby_watts: f64,
}

impl PowerProfile {
    pub fn new(operating_watts: f64, standby_watts: f64) -> Self {
        Self {
            operating_watts,
            standby_watts,
        }
    }
}

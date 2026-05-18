//! Regulatory rule data structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Frequency band with regulatory constraints
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrequencyBand {
    /// Band name (e.g., "L-band", "S-band")
    pub name: String,
    /// Lower frequency bound (Hz)
    pub lower_hz: u64,
    /// Upper frequency bound (Hz)
    pub upper_hz: u64,
    /// Regulatory body governing this band
    pub regulatory_body: String,
    /// Jurisdiction (e.g., "US", "EU", "GLOBAL")
    pub jurisdiction: String,
}

/// Power limit for a band
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerLimit {
    /// Maximum power in dBm
    pub max_power_dbm: f64,
    /// Whether this is EIRP or conducted power
    pub power_type: PowerType,
    /// Additional constraints
    pub constraints: Vec<PowerConstraint>,
}

/// Type of power measurement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerType {
    /// Effective Isotropic Radiated Power
    Eirp,
    /// Conducted power (at antenna connector)
    Conducted,
}

/// Additional power constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PowerConstraint {
    /// Duty cycle limit (percentage)
    DutyCycle { max_percent: f64 },
    /// Time limit per transmission
    TimeLimit { max_seconds: u64 },
    /// Geographic restriction
    GeographicRestriction { region: String },
}

/// License requirement for a band
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseRequirement {
    /// Type of license required
    pub license_type: LicenseType,
    /// Whether coordination is required
    pub requires_coordination: bool,
    /// License validity period
    pub validity_period: Option<LicenseValidity>,
}

/// Type of license
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LicenseType {
    /// No license required (ISM band, etc.)
    None,
    /// General license (FCC Part 15, etc.)
    General,
    /// Specific license required
    Specific { license_class: String },
    /// Amateur radio license
    Amateur { class: String },
}

/// License validity period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseValidity {
    /// License issue date
    pub issued_at: DateTime<Utc>,
    /// License expiration date
    pub expires_at: DateTime<Utc>,
}

/// Coordination requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationRequirement {
    /// Bodies that must be coordinated with
    pub coordination_bodies: Vec<String>,
    /// Coordination process
    pub process: CoordinationProcess,
}

/// Coordination process type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoordinationProcess {
    /// Pre-coordination required
    PreCoordination { notice_days: u64 },
    /// Dynamic coordination (e.g., DFS)
    Dynamic { response_time_ms: u64 },
    /// No coordination required
    None,
}

/// Complete rule set for a band
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandRules {
    /// Frequency band
    pub band: FrequencyBand,
    /// Power limits
    pub power_limits: PowerLimit,
    /// License requirements
    pub license_requirement: LicenseRequirement,
    /// Coordination requirements
    pub coordination: CoordinationRequirement,
    /// Additional constraints
    pub additional_constraints: Vec<AdditionalConstraint>,
}

/// Additional regulatory constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdditionalConstraint {
    /// Maximum bandwidth
    MaxBandwidth { max_hz: u64 },
    /// Spurious emission limits
    SpuriousEmissionLimits { limit_dbc: f64 },
    /// Occupied bandwidth requirements
    OccupiedBandwidth { min_percent: f64 },
    /// Antenna gain restrictions
    AntennaGainRestriction { max_dbi: f64 },
}

/// Complete regulatory policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryPolicy {
    /// Policy name
    pub name: String,
    /// Policy version
    pub version: String,
    /// Jurisdiction
    pub jurisdiction: String,
    /// Rules for each band
    pub band_rules: Vec<BandRules>,
    /// When this policy was issued
    pub issued_at: DateTime<Utc>,
}

impl RegulatoryPolicy {
    pub fn new(name: String, jurisdiction: String) -> Self {
        Self {
            name,
            version: "1.0".to_string(),
            jurisdiction,
            band_rules: Vec::new(),
            issued_at: Utc::now(),
        }
    }
    
    /// Add band rules
    pub fn add_band_rules(&mut self, rules: BandRules) {
        self.band_rules.push(rules);
    }
    
    /// Get rules for a specific band
    pub fn get_band_rules(&self, band_name: &str) -> Option<&BandRules> {
        self.band_rules
            .iter()
            .find(|r| r.band.name == band_name)
    }
    
    /// Check if a frequency falls within any regulated band
    pub fn find_band_for_frequency(&self, frequency_hz: u64) -> Option<&BandRules> {
        self.band_rules
            .iter()
            .find(|r| frequency_hz >= r.band.lower_hz && frequency_hz <= r.band.upper_hz)
    }
}

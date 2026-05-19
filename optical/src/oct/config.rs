// OCT Configuration

use crate::oct::standard::OctStandardVersion;
use serde::{Deserialize, Serialize};

/// OCT Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OctConfiguration {
    pub standard_version: OctStandardVersion,
    pub link_type: LinkType,
    pub modulation: Modulation,
    pub fec: FecConfiguration,
    pub arq: ArqConfiguration,
    pub target_data_rate: u64,
    pub tracking_tone: Option<TrackingTone>,
}

impl OctConfiguration {
    pub fn default_s2s() -> Self {
        Self {
            standard_version: OctStandardVersion::V4_0_0,
            link_type: LinkType::SpaceToSpace,
            modulation: Modulation::ManchesterBm16,
            fec: FecConfiguration::default_ldpc(),
            arq: ArqConfiguration::default(),
            target_data_rate: 10_000_000_000, // 10 Gbps
            tracking_tone: Some(TrackingTone::default()),
        }
    }

    pub fn default_s2t() -> Self {
        Self {
            standard_version: OctStandardVersion::V4_0_0,
            link_type: LinkType::SpaceToTerrestrial {
                atmospheric_model: AtmosphericModel::Standard,
            },
            modulation: Modulation::ManchesterBm16,
            fec: FecConfiguration::default_ldpc(),
            arq: ArqConfiguration::default(),
            target_data_rate: 10_000_000_000, // 10 Gbps
            tracking_tone: Some(TrackingTone::default()),
        }
    }
}

/// Link type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LinkType {
    SpaceToSpace,
    SpaceToTerrestrial { atmospheric_model: AtmosphericModel },
}

/// Atmospheric model for space-to-terrestrial links
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtmosphericModel {
    Standard,
    Clear,
    ModerateTurbulence,
    HighTurbulence,
    Storm,
}

/// Modulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modulation {
    OokNrz { baud_rate: u64 },
    Manchester { baud_rate: u64 },
    ManchesterBm12,
    ManchesterBm16,
}

impl Modulation {
    pub fn data_rate(&self) -> u64 {
        match self {
            Self::OokNrz { baud_rate } => *baud_rate,
            Self::Manchester { baud_rate } => *baud_rate / 2,
            Self::ManchesterBm12 => 5_000_000_000,  // 5 Gbps
            Self::ManchesterBm16 => 10_000_000_000, // 10 Gbps
        }
    }
}

/// FEC Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FecConfiguration {
    pub enabled: bool,
    pub code: FecCode,
    pub code_rate: CodeRate,
    pub block_size: usize,
}

impl FecConfiguration {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            code: FecCode::Disabled,
            code_rate: CodeRate::R1_2,
            block_size: 0,
        }
    }

    pub fn default_ldpc() -> Self {
        Self {
            enabled: true,
            code: FecCode::Ldpc5gNr {
                variant: LdpcVariant::BaseGraph1,
            },
            code_rate: CodeRate::R5_6,
            block_size: 8448,
        }
    }
}

/// FEC Code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FecCode {
    Ldpc5gNr { variant: LdpcVariant },
    Disabled,
}

/// LDPC variant (5G NR)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LdpcVariant {
    BaseGraph1,
    BaseGraph2,
}

/// Code rate for LDPC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeRate {
    R1_2,
    R2_3,
    R3_4,
    R5_6,
}

impl CodeRate {
    pub fn as_float(&self) -> f64 {
        match self {
            Self::R1_2 => 0.5,
            Self::R2_3 => 0.666,
            Self::R3_4 => 0.75,
            Self::R5_6 => 0.833,
        }
    }
}

/// ARQ Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArqConfiguration {
    pub enabled: bool,
    pub window_size: u32,
    pub timeout_ms: u32,
    pub max_retries: u32,
}

impl Default for ArqConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            window_size: 1024,
            timeout_ms: 100,
            max_retries: 5,
        }
    }
}

impl ArqConfiguration {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            window_size: 0,
            timeout_ms: 0,
            max_retries: 0,
        }
    }
}

/// Tracking tone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingTone {
    pub frequency_hz: u64,
    pub power_dbm: f64,
}

impl Default for TrackingTone {
    fn default() -> Self {
        Self {
            frequency_hz: 10_000_000, // 10 MHz
            power_dbm: -10.0,
        }
    }
}

impl TrackingTone {}

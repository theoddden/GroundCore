// SDA OCT Standard configuration
//
// The OctConfiguration parameters are SDA standard fields - modulation,
// FEC settings, ARQ on/off, data rate. The trait knows nothing vendor-specific.
// The vendor structs handle vendor-specific control protocols underneath.

use crate::{DataRate, Frequency};
use serde::{Deserialize, Serialize};

/// SDA OCT Standard version
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OctStandardVersion {
    V3_0,
    V3_1,
    V3_2,
    V4_0_0,
}

/// Link type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkType {
    S2S, // Space-to-Space (high SNR, ARQ off)
    S2T, // Space-to-Terrestrial (low SNR, ARQ on)
}

/// Modulation scheme
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Modulation {
    OokNrz { rate: BaudRate },
    Manchester { rate: BaudRate },
    ManchesterBm12, // OCT 4.0.0 burst mode 1/12 duty cycle
    ManchesterBm16, // OCT 4.0.0 burst mode 1/16 duty cycle
}

/// Baud rate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaudRate(pub u64);

/// FEC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FecConfiguration {
    pub enabled: bool,
    pub code: FecCode,
    pub code_rate: CodeRate,
}

/// FEC code (5G NR LDPC per OCT Standard)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FecCode {
    Ldpc5gNr { variant: LdpcVariant },
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

/// ARQ configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArqConfiguration {
    pub enabled: bool,
    pub max_retransmissions: u8,
    pub timeout_ms: u64,
}

/// SDA OCT Standard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OctConfiguration {
    pub standard_version: OctStandardVersion,
    pub link_type: LinkType,
    pub modulation: Modulation,
    pub fec_config: FecConfiguration,
    pub arq_config: ArqConfiguration,
    pub target_data_rate: DataRate,
    pub tracking_tone_frequency: Option<Frequency>, // 40 kHz or 50 kHz
}

impl OctConfiguration {
    /// Default S2S configuration (high SNR, ARQ off)
    pub fn default_s2s() -> Self {
        Self {
            standard_version: OctStandardVersion::V4_0_0,
            link_type: LinkType::S2S,
            modulation: Modulation::ManchesterBm12,
            fec_config: FecConfiguration {
                enabled: true,
                code: FecCode::Ldpc5gNr { variant: LdpcVariant::BaseGraph1 },
                code_rate: CodeRate::R2_3,
            },
            arq_config: ArqConfiguration {
                enabled: false,
                max_retransmissions: 0,
                timeout_ms: 0,
            },
            target_data_rate: DataRate(2_500_000_000), // 2.5 Gbps
            tracking_tone_frequency: Some(Frequency(40_000)), // 40 kHz
        }
    }

    /// Default S2T configuration (low SNR, ARQ on)
    pub fn default_s2t() -> Self {
        Self {
            standard_version: OctStandardVersion::V4_0_0,
            link_type: LinkType::S2T,
            modulation: Modulation::Manchester,
            fec_config: FecConfiguration {
                enabled: true,
                code: FecCode::Ldpc5gNr { variant: LdpcVariant::BaseGraph1 },
                code_rate: CodeRate::R1_2, // Lower rate for S2T
            },
            arq_config: ArqConfiguration {
                enabled: true,
                max_retransmissions: 5,
                timeout_ms: 100,
            },
            target_data_rate: DataRate(1_000_000_000), // 1 Gbps
            tracking_tone_frequency: Some(Frequency(50_000)), // 50 kHz
        }
    }
}

impl Default for OctConfiguration {
    fn default() -> Self {
        Self::default_s2s()
    }
}

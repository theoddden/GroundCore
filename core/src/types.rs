//! Core types for Ground Station Core

use serde::{Deserialize, Serialize};

/// Satellite identifier
pub type SatelliteId = String;

/// Ground station identifier
pub type StationId = String;

/// Pass identifier
pub type PassId = String;

/// Customer identifier
pub type CustomerId = String;

/// Frequency in Hz
pub type Frequency = u64;

/// Data rate in bits per second
pub type DataRate = u64;

/// Bytes
pub type Bytes = u64;

/// Frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrequencyBand {
    LBand,
    SBand,
    CBand,
    XBand,
    KuBand,
    KaBand,
}

impl FrequencyBand {
    pub fn name(&self) -> &str {
        match self {
            FrequencyBand::LBand => "L-band",
            FrequencyBand::SBand => "S-band",
            FrequencyBand::CBand => "C-band",
            FrequencyBand::XBand => "X-band",
            FrequencyBand::KuBand => "Ku-band",
            FrequencyBand::KaBand => "Ka-band",
        }
    }
}

// OCT Standard version handling

use serde::{Deserialize, Serialize};

/// OCT Standard version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OctStandardVersion {
    V3_0_0,
    V3_0_1,
    V3_1_0,
    V3_2_0,
    V4_0_0,
}

impl OctStandardVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V3_0_0 => "3.0.0",
            Self::V3_0_1 => "3.0.1",
            Self::V3_1_0 => "3.1.0",
            Self::V3_2_0 => "3.2.0",
            Self::V4_0_0 => "4.0.0",
        }
    }

    pub fn supports_ldpc(&self) -> bool {
        matches!(self, Self::V3_1_0 | Self::V3_2_0 | Self::V4_0_0)
    }

    pub fn supports_manchester_bm16(&self) -> bool {
        matches!(self, Self::V4_0_0)
    }

    pub fn max_data_rate(&self) -> u64 {
        match self {
            Self::V3_0_0 | Self::V3_0_1 => 2_500_000_000, // 2.5 Gbps
            Self::V3_1_0 | Self::V3_2_0 => 5_000_000_000, // 5 Gbps
            Self::V4_0_0 => 10_000_000_000,               // 10 Gbps
        }
    }
}

/// OCT Standard descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OctStandard {
    pub version: OctStandardVersion,
    pub specification_url: String,
    pub release_date: chrono::DateTime<chrono::Utc>,
}

impl OctStandard {
    pub fn v3_0_0() -> Self {
        Self {
            version: OctStandardVersion::V3_0_0,
            specification_url: "https://sda.org/oct/3.0.0".to_string(),
            release_date: chrono::DateTime::parse_from_rfc3339("2020-06-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    pub fn v3_1_0() -> Self {
        Self {
            version: OctStandardVersion::V3_1_0,
            specification_url: "https://sda.org/oct/3.1.0".to_string(),
            release_date: chrono::DateTime::parse_from_rfc3339("2021-03-15T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    pub fn v3_2_0() -> Self {
        Self {
            version: OctStandardVersion::V3_2_0,
            specification_url: "https://sda.org/oct/3.2.0".to_string(),
            release_date: chrono::DateTime::parse_from_rfc3339("2022-01-10T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    pub fn v4_0_0() -> Self {
        Self {
            version: OctStandardVersion::V4_0_0,
            specification_url: "https://sda.org/oct/4.0.0".to_string(),
            release_date: chrono::DateTime::parse_from_rfc3339("2023-08-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }
}

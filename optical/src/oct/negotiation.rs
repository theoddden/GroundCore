// Standard version negotiation

use crate::oct::standard::OctStandardVersion;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NegotiationError {
    #[error("No common OCT standard version between terminals")]
    NoCommonVersion,
}

/// Negotiate the highest mutually-supported OCT standard version
pub fn negotiate_version(
    local: &[OctStandardVersion],
    peer: &[OctStandardVersion],
) -> Result<OctStandardVersion, NegotiationError> {
    local
        .iter()
        .filter(|v| peer.contains(v))
        .max()
        .copied()
        .ok_or(NegotiationError::NoCommonVersion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_version_common() {
        let local = vec![
            OctStandardVersion::V3_0_0,
            OctStandardVersion::V3_1_0,
            OctStandardVersion::V4_0_0,
        ];
        let peer = vec![OctStandardVersion::V3_1_0, OctStandardVersion::V3_2_0];

        let result = negotiate_version(&local, &peer).unwrap();
        assert_eq!(result, OctStandardVersion::V3_1_0);
    }

    #[test]
    fn test_negotiate_version_no_common() {
        let local = vec![OctStandardVersion::V3_0_0];
        let peer = vec![OctStandardVersion::V4_0_0];

        let result = negotiate_version(&local, &peer);
        assert!(result.is_err());
    }

    #[test]
    fn test_negotiate_version_highest_common() {
        let local = vec![
            OctStandardVersion::V3_0_0,
            OctStandardVersion::V3_1_0,
            OctStandardVersion::V4_0_0,
        ];
        let peer = vec![
            OctStandardVersion::V3_0_0,
            OctStandardVersion::V3_1_0,
            OctStandardVersion::V3_2_0,
            OctStandardVersion::V4_0_0,
        ];

        let result = negotiate_version(&local, &peer).unwrap();
        assert_eq!(result, OctStandardVersion::V4_0_0);
    }
}

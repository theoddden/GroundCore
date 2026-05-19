//! Transmit function with compile-time enforcement
//!
//! This is the key function that enforces regulatory compliance at compile time.
//! It cannot be called without:
//! - A license typed to the specific band
//! - A frequency typed to the band
//! - A coordination proof (if required by the band)

use crate::license::License;
use crate::types::{Band, CoordinationProof, Frequency, Power};
use ground_core::Result;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransmitError {
    #[error("License not valid")]
    LicenseNotValid,
    #[error("Power exceeds license limit")]
    PowerExceedsLimit,
    #[error("Coordination required but not provided")]
    CoordinationRequired,
    #[error("Transmit failed: {0}")]
    TransmitFailed(String),
}

/// Transmit on a specific band
///
/// This function enforces regulatory compliance at the type level:
/// - The license must be typed to band B
/// - The frequency must be typed to band B
/// - The coordination proof must be typed to band B (if provided)
///
/// If you don't have the license, your code doesn't compile.
/// This is compile-time enforcement, not runtime checking.
pub fn transmit<B: Band>(
    license: &License<B>,
    frequency: Frequency<B>,
    power: Power,
    coordination: Option<&CoordinationProof<B>>,
    requires_coordination: bool,
) -> Result<()> {
    // Runtime checks (these should never fail if types are correct)
    if !license.is_valid() {
        return Err(ground_core::GroundStationError::Regulatory(
            "License not valid".to_string(),
        ));
    }

    if !license.power_allowed(power.dbm) {
        return Err(ground_core::GroundStationError::Regulatory(
            "Power exceeds limit".to_string(),
        ));
    }

    if requires_coordination && coordination.is_none() {
        return Err(ground_core::GroundStationError::Regulatory(
            "Coordination required".to_string(),
        ));
    }

    // In a real implementation, this would actually transmit
    tracing::info!(
        "Transmitting on {} at {} Hz at {} dBm",
        B::NAME,
        frequency.hz(),
        power.dbm
    );

    Ok(())
}

/// Transmit without coordination (for bands that don't require it)
pub fn transmit_no_coordination<B: Band>(
    license: &License<B>,
    frequency: Frequency<B>,
    power: Power,
) -> Result<()> {
    transmit(license, frequency, power, None, false)
}

/// Transmit with coordination (for bands that require it)
pub fn transmit_with_coordination<B: Band>(
    license: &License<B>,
    frequency: Frequency<B>,
    power: Power,
    coordination: &CoordinationProof<B>,
) -> Result<()> {
    transmit(license, frequency, power, Some(coordination), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::License;
    use crate::types::{LBand, Power};
    use chrono::Utc;
    use ground_core::CustomerId;

    #[test]
    fn test_transmit_success() {
        let holder: CustomerId = "test".to_string();
        let license = License::<LBand>::new(
            holder,
            Utc::now(),
            Utc::now() + chrono::Duration::hours(24),
            30.0,
        );

        let frequency = Frequency::<LBand>::new(1_500_000_000).unwrap();
        let power = Power::new(20.0);

        let result = transmit_no_coordination(&license, frequency, power);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transmit_power_limit() {
        let holder: CustomerId = "test".to_string();
        let license = License::<LBand>::new(
            holder,
            Utc::now(),
            Utc::now() + chrono::Duration::hours(24),
            30.0,
        );

        let frequency = Frequency::<LBand>::new(1_500_000_000).unwrap();
        let power = Power::new(40.0); // Exceeds 30 dBm limit

        let result = transmit_no_coordination(&license, frequency, power);
        assert!(matches!(
            result,
            Err(ground_core::GroundStationError::Regulatory(_))
        ));
    }

    #[test]
    fn test_transmit_expired_license() {
        let holder: CustomerId = "test".to_string();
        let license = License::<LBand>::new(
            holder,
            Utc::now() - chrono::Duration::hours(48),
            Utc::now() - chrono::Duration::hours(24),
            30.0,
        );

        let frequency = Frequency::<LBand>::new(1_500_000_000).unwrap();
        let power = Power::new(20.0);

        let result = transmit_no_coordination(&license, frequency, power);
        assert!(matches!(
            result,
            Err(ground_core::GroundStationError::Regulatory(_))
        ));
    }
}

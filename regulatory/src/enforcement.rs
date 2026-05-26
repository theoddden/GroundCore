//! Compliance checking and enforcement
//!
//! Runtime verification that complements the compile-time type enforcement.
//! While the type system catches most violations at compile time, this
//! provides additional runtime checks for dynamic scenarios.

use crate::license::LicenseStore;
use crate::types::Band;
use chrono::Utc;
use ground_core::CustomerId;
use serde::{Deserialize, Serialize};

/// Compliance check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub compliant: bool,
    pub violations: Vec<ComplianceViolation>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// Compliance violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub violation_type: ViolationType,
    pub description: String,
    pub severity: Severity,
}

/// Type of violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    /// No license for required band
    MissingLicense { band: String },
    /// License expired
    ExpiredLicense { holder: CustomerId, band: String },
    /// Power exceeds license limit
    PowerExceedsLimit {
        band: String,
        requested_dbm: f64,
        allowed_dbm: f64,
    },
    /// Coordination not obtained
    MissingCoordination { band: String },
    /// Transmitting outside licensed band
    FrequencyOutsideBand { band: String, frequency: u64 },
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Compliance checker
pub struct ComplianceChecker {
    license_store: LicenseStore,
}

impl ComplianceChecker {
    pub fn new(license_store: LicenseStore) -> Self {
        Self { license_store }
    }

    /// Check if a holder can transmit on a specific band
    pub fn check_transmit_permission<B: Band>(
        &self,
        holder: &CustomerId,
        frequency: u64,
        power_dbm: f64,
    ) -> ComplianceResult {
        let mut violations = Vec::new();

        // Check if license exists and is valid
        let license = self.license_store.get_license::<B>(holder);

        if license.is_none() {
            violations.push(ComplianceViolation {
                violation_type: ViolationType::MissingLicense {
                    band: B::NAME.to_string(),
                },
                description: format!("No license found for {} band", B::NAME),
                severity: Severity::Critical,
            });
        } else if let Some(license) = license {
            if !license.is_valid() {
                violations.push(ComplianceViolation {
                    violation_type: ViolationType::ExpiredLicense {
                        holder: holder.clone(),
                        band: B::NAME.to_string(),
                    },
                    description: format!("License for {} band has expired", B::NAME),
                    severity: Severity::Critical,
                });
            }

            if !license.power_allowed(power_dbm) {
                violations.push(ComplianceViolation {
                    violation_type: ViolationType::PowerExceedsLimit {
                        band: B::NAME.to_string(),
                        requested_dbm: power_dbm,
                        allowed_dbm: license.max_power_dbm,
                    },
                    description: format!(
                        "Power {} dBm exceeds license limit {} dBm",
                        power_dbm, license.max_power_dbm
                    ),
                    severity: Severity::High,
                });
            }
        }

        // Check if frequency is within band
        if frequency < B::MIN_FREQUENCY || frequency > B::MAX_FREQUENCY {
            violations.push(ComplianceViolation {
                violation_type: ViolationType::FrequencyOutsideBand {
                    band: B::NAME.to_string(),
                    frequency,
                },
                description: format!(
                    "Frequency {} Hz is outside {} band ({}-{} Hz)",
                    frequency,
                    B::NAME,
                    B::MIN_FREQUENCY,
                    B::MAX_FREQUENCY
                ),
                severity: Severity::Critical,
            });
        }

        ComplianceResult {
            compliant: violations.is_empty(),
            violations,
            checked_at: Utc::now(),
        }
    }

    /// Get compliance report for a holder across all bands
    pub fn get_holder_compliance(&self, holder: &CustomerId) -> ComplianceReport {
        let license_keys = self.license_store.get_holder_licenses(holder);

        let mut total_licenses = 0;
        let mut valid_licenses = 0;
        let mut expired_licenses = 0;

        for key in &license_keys {
            total_licenses += 1;
            match self.license_store.check_license_validity(key) {
                Some(true) => valid_licenses += 1,
                Some(false) => expired_licenses += 1,
                None => {
                    tracing::warn!("Could not determine validity of license {}", key);
                }
            }
        }

        let overall_compliant = total_licenses > 0 && expired_licenses == 0 && valid_licenses > 0;

        ComplianceReport {
            holder: holder.clone(),
            total_licenses,
            valid_licenses,
            expired_licenses,
            overall_compliant,
            checked_at: Utc::now(),
        }
    }
}

/// Compliance report for a holder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub holder: CustomerId,
    pub total_licenses: usize,
    pub valid_licenses: usize,
    pub expired_licenses: usize,
    pub overall_compliant: bool,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::License;
    use crate::license::LicenseStore;
    use crate::types::LBand;
    use chrono::Utc;
    use ground_core::CustomerId;

    #[test]
    fn test_compliance_check_success() {
        let mut store = LicenseStore::new();
        let holder = CustomerId::from("test");

        let license = License::<LBand>::new(
            holder.clone(),
            Utc::now(),
            Utc::now() + chrono::Duration::hours(24),
            30.0,
        );

        let key = format!("{}:{}", holder, LBand::NAME);
        store.add_license(key, serde_json::to_value(&license).unwrap());

        let checker = ComplianceChecker::new(store);
        let result = checker.check_transmit_permission::<LBand>(&holder, 1_500_000_000, 20.0);

        assert!(result.compliant);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_compliance_check_missing_license() {
        let store = LicenseStore::new();
        let holder = CustomerId::from("test");

        let checker = ComplianceChecker::new(store);
        let result = checker.check_transmit_permission::<LBand>(&holder, 1_500_000_000, 20.0);

        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(
            result.violations[0].violation_type,
            ViolationType::MissingLicense { .. }
        ));
    }
}

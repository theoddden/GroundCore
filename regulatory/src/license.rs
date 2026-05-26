//! License management with type-level enforcement
//!
//! Licenses are typed to specific bands. You cannot use an L-band license
//! to transmit on S-band - the type system prevents it.

use crate::types::Band;
use chrono::{DateTime, Utc};
use ground_core::CustomerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

/// License for a specific band
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License<B: Band> {
    pub holder: CustomerId,
    pub band: PhantomData<B>,
    pub valid_from: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub max_power_dbm: f64,
}

impl<B: Band> License<B> {
    pub fn new(
        holder: CustomerId,
        valid_from: DateTime<Utc>,
        valid_until: DateTime<Utc>,
        max_power_dbm: f64,
    ) -> Self {
        Self {
            holder,
            band: PhantomData,
            valid_from,
            valid_until,
            max_power_dbm,
        }
    }

    /// Check if this license is currently valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        now >= self.valid_from && now < self.valid_until
    }

    /// Get the remaining validity duration
    pub fn remaining_validity(&self) -> Option<chrono::Duration> {
        let now = Utc::now();
        if now < self.valid_until {
            Some(self.valid_until - now)
        } else {
            None
        }
    }

    /// Check if a power level is within license limits
    pub fn power_allowed(&self, power_dbm: f64) -> bool {
        power_dbm <= self.max_power_dbm
    }
}

/// License store for managing licenses
pub struct LicenseStore {
    licenses: HashMap<String, serde_json::Value>,
}

impl Default for LicenseStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseStore {
    pub fn new() -> Self {
        Self {
            licenses: HashMap::new(),
        }
    }

    /// Add a license (serialized)
    pub fn add_license(&mut self, key: String, license: serde_json::Value) {
        self.licenses.insert(key, license);
    }

    /// Get a license for a specific band
    pub fn get_license<B: Band>(&self, holder: &CustomerId) -> Option<License<B>> {
        let key = format!("{}:{}", holder, B::NAME);
        self.licenses
            .get(&key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// Check if a holder has a valid license for a band
    pub fn has_valid_license<B: Band>(&self, holder: &CustomerId) -> bool {
        self.get_license::<B>(holder)
            .map(|license| license.is_valid())
            .unwrap_or(false)
    }

    /// Check whether a license stored under `key` is currently valid.
    /// Returns `Some(true)` if valid, `Some(false)` if expired, `None` if the
    /// key doesn't exist or the timestamp cannot be parsed.
    pub fn check_license_validity(&self, key: &str) -> Option<bool> {
        let value = self.licenses.get(key)?;
        let until_str = value.get("valid_until")?.as_str()?;
        let until = chrono::DateTime::parse_from_rfc3339(until_str)
            .or_else(|_| chrono::DateTime::parse_from_rfc3339(&until_str.replace(" ", "T")))
            .ok()?;
        Some(until > chrono::Utc::now())
    }

    /// Remove a license
    pub fn remove_license(&mut self, holder: &CustomerId, band_name: &str) {
        let key = format!("{}:{}", holder, band_name);
        self.licenses.remove(&key);
    }

    /// Get all licenses for a holder
    pub fn get_holder_licenses(&self, holder: &CustomerId) -> Vec<String> {
        self.licenses
            .keys()
            .filter(|k| k.starts_with(&format!("{}:", holder)))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LBand;
    use ground_core::CustomerId;

    #[test]
    fn test_license_validity() {
        let holder = CustomerId::from("test-customer");
        let now = Utc::now();
        let future = now + chrono::Duration::hours(24);

        let license = License::<LBand>::new(holder.clone(), now, future, 30.0);
        assert!(license.is_valid());

        let expired_license = License::<LBand>::new(
            holder.clone(),
            now - chrono::Duration::hours(48),
            now - chrono::Duration::hours(24),
            30.0,
        );
        assert!(!expired_license.is_valid());
    }

    #[test]
    fn test_license_store() {
        let mut store = LicenseStore::new();
        let holder = CustomerId::from("test-customer");

        let license = License::<LBand>::new(
            holder.clone(),
            Utc::now(),
            Utc::now() + chrono::Duration::hours(24),
            30.0,
        );

        let key = format!("{}:{}", holder, LBand::NAME);
        store.add_license(key.clone(), serde_json::to_value(&license).unwrap());

        assert!(store.has_valid_license::<LBand>(&holder));
    }
}

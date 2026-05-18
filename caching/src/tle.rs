//! TLE caching with atomic refresh
//!
//! TLE refresh is atomic: you get a consistent snapshot of the constellation
//! state at a known instant, rather than individual fetches across several seconds.

use chrono::{DateTime, Utc};
use ground_core::SatelliteId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cache key for TLE entries
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TleCacheKey {
    pub satellite_id: SatelliteId,
    pub source: String,
}

/// Cached TLE data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTle {
    pub line1: String,
    pub line2: String,
    pub epoch: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
}

/// TLE cache with batch refresh support
pub struct TleCache {
    entries: HashMap<TleCacheKey, CachedTle>,
    max_age_hours: i64,
}

impl TleCache {
    pub fn new(max_age_hours: i64) -> Self {
        Self {
            entries: HashMap::new(),
            max_age_hours,
        }
    }

    /// Get a cached TLE
    pub fn get(&self, key: &TleCacheKey) -> Option<&CachedTle> {
        self.entries.get(key)
    }

    /// Check if a TLE needs refresh
    pub fn needs_refresh(&self, key: &TleCacheKey) -> bool {
        match self.get(key) {
            Some(tle) => {
                let age = Utc::now() - tle.fetched_at;
                age.num_hours() > self.max_age_hours
            }
            None => true,
        }
    }

    /// Batch refresh multiple TLEs atomically
    pub fn batch_refresh(&mut self, tles: Vec<(TleCacheKey, CachedTle)>) {
        for (key, tle) in tles {
            self.entries.insert(key, tle);
        }
    }

    /// Invalidate a specific TLE
    pub fn invalidate(&mut self, key: &TleCacheKey) {
        self.entries.remove(key);
    }

    /// Invalidate all TLEs for a satellite
    pub fn invalidate_satellite(&mut self, satellite_id: &SatelliteId) {
        self.entries
            .retain(|key, _| &key.satellite_id != satellite_id);
    }

    /// Invalidate all TLEs
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    /// Get all satellite IDs in cache
    pub fn satellite_ids(&self) -> Vec<SatelliteId> {
        self.entries
            .keys()
            .map(|k| k.satellite_id.clone())
            .collect()
    }

    /// Get cache statistics
    pub fn stats(&self) -> TleCacheStats {
        TleCacheStats {
            entry_count: self.entries.len(),
            satellite_count: self.satellite_ids().len(),
        }
    }
}

/// TLE cache statistics
#[derive(Debug, Clone)]
pub struct TleCacheStats {
    pub entry_count: usize,
    pub satellite_count: usize,
}

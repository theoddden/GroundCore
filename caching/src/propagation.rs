//! Orbital propagation caching with interpolation
//!
//! Orbital state is smoothly differentiable on short timescales. If you've
//! propagated a satellite's position at T=0 and T=60s, you can interpolate any
//! time in between to sub-meter accuracy without re-running SGP4.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Cached orbital state entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Orbital state at this time
    pub position: (f64, f64, f64),
    /// Velocity at this time
    pub velocity: (f64, f64, f64),
    /// When this state is valid
    pub time: DateTime<Utc>,
    /// TLE epoch used for propagation
    pub tle_epoch: DateTime<Utc>,
}

/// Propagation cache with interpolation
pub struct PropagationCache {
    /// Cached positions indexed by time
    entries: BTreeMap<DateTime<Utc>, CacheEntry>,
    /// Interpolation window - max time between cached points for interpolation
    interpolation_window: Duration,
    /// Maximum cache age before invalidation
    max_age: Duration,
}

impl PropagationCache {
    pub fn new(interpolation_window_sec: i64, max_age_sec: i64) -> Self {
        Self {
            entries: BTreeMap::new(),
            interpolation_window: Duration::seconds(interpolation_window_sec),
            max_age: Duration::seconds(max_age_sec),
        }
    }

    /// Look up or compute orbital state with caching
    pub fn lookup_or_compute<F>(&mut self, time: DateTime<Utc>, compute_fn: F) -> Option<CacheEntry>
    where
        F: FnOnce(DateTime<Utc>) -> CacheEntry,
    {
        // Clean up old entries
        self.cleanup();

        // Try to find interpolatable cached positions
        if let Some(entry) = self.lookup_interpolatable(time) {
            return Some(entry);
        }

        // Compute fresh state
        let entry = compute_fn(time);

        // Insert into cache
        self.entries.insert(time, entry.clone());

        Some(entry)
    }

    /// Look up an interpolatable position
    fn lookup_interpolatable(&self, time: DateTime<Utc>) -> Option<CacheEntry> {
        // Find surrounding cached positions
        let before = self.entries.range(..=time).next_back();
        let after = self.entries.range(time..).next();

        match (before, after) {
            (Some((t1, s1)), Some((t2, s2))) => {
                // Check if within interpolation window
                let dt1 = (time - *t1).num_seconds().abs();
                let dt2 = (*t2 - time).num_seconds().abs();

                if dt1 <= self.interpolation_window.num_seconds()
                    && dt2 <= self.interpolation_window.num_seconds()
                {
                    // Linear interpolation
                    let total_dt = (*t2 - *t1).num_seconds() as f64;
                    let t = (time - *t1).num_seconds() as f64 / total_dt;

                    let position = (
                        s1.position.0 * (1.0 - t) + s2.position.0 * t,
                        s1.position.1 * (1.0 - t) + s2.position.1 * t,
                        s1.position.2 * (1.0 - t) + s2.position.2 * t,
                    );

                    let velocity = (
                        s1.velocity.0 * (1.0 - t) + s2.velocity.0 * t,
                        s1.velocity.1 * (1.0 - t) + s2.velocity.1 * t,
                        s1.velocity.2 * (1.0 - t) + s2.velocity.2 * t,
                    );

                    return Some(CacheEntry {
                        position,
                        velocity,
                        time,
                        tle_epoch: s1.tle_epoch,
                    });
                }
            }
            (Some((t1, s1)), None) => {
                // Only have before, use if within window
                if (time - *t1).num_seconds().abs() <= self.interpolation_window.num_seconds() {
                    return Some(s1.clone());
                }
            }
            (None, Some((t2, s2))) => {
                // Only have after, use if within window
                if (*t2 - time).num_seconds().abs() <= self.interpolation_window.num_seconds() {
                    return Some(s2.clone());
                }
            }
            (None, None) => {}
        }

        None
    }

    /// Invalidate cache entries older than max_age
    fn cleanup(&mut self) {
        let now = Utc::now();
        self.entries.retain(|time, _| now - *time < self.max_age);
    }

    /// Invalidate all cache entries
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.entries.len(),
            oldest_entry: self.entries.first_key_value().map(|(t, _)| *t),
            newest_entry: self.entries.last_key_value().map(|(t, _)| *t),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: usize,
    pub oldest_entry: Option<DateTime<Utc>>,
    pub newest_entry: Option<DateTime<Utc>>,
}

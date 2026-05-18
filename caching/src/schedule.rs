//! Schedule fragment caching
//!
//! The full schedule optimization runs every few minutes. Most of the schedule
//! doesn't change between runs. Caching schedule fragments lets incremental
//! re-optimization complete in <500ms instead of re-running the full annealing.

use chrono::{DateTime, Utc};
use ground_core::PassId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Immutable schedule fragment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleFragment {
    /// Pass IDs in this fragment
    pub pass_ids: Vec<PassId>,
    /// When this fragment was committed
    pub committed_at: DateTime<Utc>,
    /// When this fragment becomes mutable again
    pub immutable_until: DateTime<Utc>,
    /// Hash of the fragment state
    pub hash: String,
}

impl ScheduleFragment {
    pub fn new(pass_ids: Vec<PassId>, immutable_duration_sec: i64) -> Self {
        let now = Utc::now();
        let hash = Self::compute_hash(&pass_ids);

        Self {
            pass_ids,
            committed_at: now,
            immutable_until: now + chrono::Duration::seconds(immutable_duration_sec),
            hash,
        }
    }

    /// Check if this fragment is currently immutable
    pub fn is_immutable(&self) -> bool {
        Utc::now() < self.immutable_until
    }

    /// Check if a pass is in this fragment
    pub fn contains(&self, pass_id: &PassId) -> bool {
        self.pass_ids.contains(pass_id)
    }

    /// Compute hash of pass IDs
    fn compute_hash(pass_ids: &[PassId]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for pass_id in pass_ids {
            hasher.update(pass_id.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Schedule fragment cache
pub struct ScheduleFragmentCache {
    fragments: HashMap<String, ScheduleFragment>,
    max_fragments: usize,
}

impl ScheduleFragmentCache {
    pub fn new(max_fragments: usize) -> Self {
        Self {
            fragments: HashMap::new(),
            max_fragments,
        }
    }

    /// Add a fragment
    pub fn add_fragment(&mut self, fragment: ScheduleFragment) {
        self.fragments.insert(fragment.hash.clone(), fragment);

        // Prune old fragments
        while self.fragments.len() > self.max_fragments {
            // Remove oldest fragment
            if let Some(oldest) = self.fragments.values().min_by_key(|f| f.committed_at) {
                self.fragments.remove(&oldest.hash.clone());
            }
        }
    }

    /// Get a fragment by hash
    pub fn get_fragment(&self, hash: &str) -> Option<&ScheduleFragment> {
        self.fragments.get(hash)
    }

    /// Get fragment containing a specific pass
    pub fn get_fragment_for_pass(&self, pass_id: &PassId) -> Option<&ScheduleFragment> {
        self.fragments
            .values()
            .find(|f| f.contains(pass_id) && f.is_immutable())
    }

    /// Invalidate expired fragments
    pub fn invalidate_expired(&mut self) {
        self.fragments.retain(|_, f| f.is_immutable());
    }

    /// Get cache statistics
    pub fn stats(&self) -> ScheduleCacheStats {
        let total = self.fragments.len();
        let immutable = self.fragments.values().filter(|f| f.is_immutable()).count();

        ScheduleCacheStats {
            total_fragments: total,
            immutable_fragments: immutable,
            mutable_fragments: total - immutable,
        }
    }
}

/// Schedule cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleCacheStats {
    pub total_fragments: usize,
    pub immutable_fragments: usize,
    pub mutable_fragments: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_creation() {
        let pass_ids = vec!["pass1".to_string(), "pass2".to_string()];
        let fragment = ScheduleFragment::new(pass_ids, 300);

        assert!(fragment.is_immutable());
        assert!(fragment.contains(&"pass1".to_string()));
    }

    #[test]
    fn test_fragment_cache() {
        let mut cache = ScheduleFragmentCache::new(10);
        let pass_ids = vec!["pass1".to_string()];
        let fragment = ScheduleFragment::new(pass_ids, 300);

        cache.add_fragment(fragment.clone());
        assert_eq!(cache.stats().total_fragments, 1);

        let retrieved = cache.get_fragment(&fragment.hash);
        assert!(retrieved.is_some());
    }
}

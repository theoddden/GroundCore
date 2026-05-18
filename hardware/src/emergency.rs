//! Emergency shard pool for failover
//!
//! Pre-allocated emergency shards that can be claimed instantly during
//! failover without heap allocation. This is the same pattern as Float
//! Protocols' deadzone shard, applied to ground stations.

use bumpalo::Bump;
use chrono::Utc;
use ground_core::Result;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Pre-allocated emergency shard
pub struct EmergencyShard {
    /// Arena for no-heap allocations
    arena: Bump,
    /// Whether this shard is currently in use
    in_use: AtomicBool,
    /// When this shard was last claimed
    last_claimed: AtomicUsize, // Unix timestamp
    /// Shard size
    size: usize,
}

impl EmergencyShard {
    /// Create a new emergency shard with pre-allocated memory
    pub fn new(size: usize) -> Self {
        Self {
            arena: Bump::with_capacity(size),
            in_use: AtomicBool::new(false),
            last_claimed: AtomicUsize::new(0),
            size,
        }
    }

    /// Claim this shard (no allocation, just atomic swap)
    pub fn claim(&self) -> Result<&Bump> {
        let was_in_use = self.in_use.swap(true, Ordering::SeqCst);
        if was_in_use {
            return Err(ground_core::GroundStationError::Hardware(
                "Emergency shard already in use".to_string(),
            ));
        }

        self.last_claimed
            .store(Utc::now().timestamp() as usize, Ordering::SeqCst);

        Ok(&self.arena)
    }

    /// Release this shard back to the pool
    pub fn release(&mut self) {
        self.in_use.store(false, Ordering::SeqCst);
        self.arena.reset(); // Reset arena for reuse
    }

    /// Check if this shard is available
    pub fn is_available(&self) -> bool {
        !self.in_use.load(Ordering::SeqCst)
    }

    /// Get the size of this shard
    pub fn size(&self) -> usize {
        self.size
    }
}

/// Pool of pre-allocated emergency shards
pub struct EmergencyShardPool {
    /// Available shards
    shards: Vec<EmergencyShard>,
    /// Total pool size
    total_size: usize,
}

impl EmergencyShardPool {
    /// Create a new emergency shard pool
    pub fn new(shard_count: usize, shard_size: usize) -> Self {
        let shards = (0..shard_count)
            .map(|_| EmergencyShard::new(shard_size))
            .collect();

        Self {
            shards,
            total_size: shard_count * shard_size,
        }
    }

    /// Claim an available emergency shard
    pub fn claim(&self) -> Result<&EmergencyShard> {
        for shard in &self.shards {
            if shard.is_available() {
                shard.claim()?;
                return Ok(shard);
            }
        }

        Err(ground_core::GroundStationError::Hardware(
            "No emergency shards available".to_string(),
        ))
    }

    /// Release a shard back to the pool
    pub fn release(&mut self, shard: &mut EmergencyShard) {
        shard.release();
    }

    /// Get the number of available shards
    pub fn available_count(&self) -> usize {
        self.shards.iter().filter(|s| s.is_available()).count()
    }

    /// Get the total number of shards
    pub fn total_count(&self) -> usize {
        self.shards.len()
    }

    /// Get the total pool size in bytes
    pub fn total_size(&self) -> usize {
        self.total_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_shard_claim_release() {
        let pool = EmergencyShardPool::new(3, 1024);

        assert_eq!(pool.available_count(), 3);

        let shard = pool.claim().unwrap();
        assert_eq!(pool.available_count(), 2);

        pool.release(shard);
        assert_eq!(pool.available_count(), 3);
    }

    #[test]
    fn test_emergency_shard_exhaustion() {
        let pool = EmergencyShardPool::new(1, 1024);

        let _shard1 = pool.claim().unwrap();
        assert_eq!(pool.available_count(), 0);

        let result = pool.claim();
        assert!(result.is_err());
    }
}

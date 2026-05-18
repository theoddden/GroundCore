//! Generic LRU cache with time-based invalidation

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Generic LRU cache entry
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    last_accessed: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

/// Generic LRU cache
pub struct LruCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    max_size: usize,
}

impl<K, V> LruCache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
        }
    }

    /// Get a value from the cache
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = Utc::now();
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Put a value into the cache
    pub fn put(&mut self, key: K, value: V) {
        // Evict if at capacity
        if self.entries.len() >= self.max_size {
            self.evict_lru();
        }

        self.entries.insert(
            key,
            CacheEntry {
                value,
                last_accessed: Utc::now(),
                created_at: Utc::now(),
            },
        );
    }

    /// Remove a value from the cache
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|e| e.value)
    }

    /// Evict the least recently used entry
    fn evict_lru(&mut self) {
        if let Some(lru_key) = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.last_accessed)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&lru_key);
        }
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Time-based cache with TTL
pub struct TimeBasedCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    ttl_seconds: i64,
}

impl<K, V> TimeBasedCache<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            entries: HashMap::new(),
            ttl_seconds,
        }
    }

    /// Get a value from the cache
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get_mut(key) {
            // Check if expired
            if (Utc::now() - entry.created_at).num_seconds() > self.ttl_seconds {
                self.entries.remove(key);
                return None;
            }
            entry.last_accessed = Utc::now();
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Put a value into the cache
    pub fn put(&mut self, key: K, value: V) {
        self.entries.insert(
            key,
            CacheEntry {
                value,
                last_accessed: Utc::now(),
                created_at: Utc::now(),
            },
        );
    }

    /// Remove expired entries
    pub fn remove_expired(&mut self) {
        let now = Utc::now();
        self.entries
            .retain(|_, e| (now - e.created_at).num_seconds() <= self.ttl_seconds);
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

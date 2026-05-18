//! Caching infrastructure for Ground Station Core
//!
//! This provides foundational caching primitives used throughout the system:
//! - Orbital propagation caching with interpolation
//! - TLE refresh caching
//! - Schedule fragment caching
//! - Generic LRU cache with time-based invalidation

pub mod generic;
pub mod propagation;
pub mod schedule;
pub mod tle;

pub use generic::{LruCache, TimeBasedCache};
pub use propagation::{CacheEntry, PropagationCache};
pub use schedule::{ScheduleFragment, ScheduleFragmentCache};
pub use tle::{TleCache, TleCacheKey};

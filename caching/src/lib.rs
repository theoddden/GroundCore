//! Caching infrastructure for Ground Station Core
//!
//! This provides foundational caching primitives used throughout the system:
//! - Orbital propagation caching with interpolation
//! - TLE refresh caching
//! - Schedule fragment caching
//! - Generic LRU cache with time-based invalidation

pub mod propagation;
pub mod tle;
pub mod schedule;
pub mod generic;

pub use propagation::{PropagationCache, CacheEntry};
pub use tle::{TleCache, TleCacheKey};
pub use schedule::{ScheduleFragment, ScheduleFragmentCache};
pub use generic::{LruCache, TimeBasedCache};

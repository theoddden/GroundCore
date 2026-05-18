//! Hardware management with pass-isolated shards
//!
//! This implements the sharding pattern for hardware allocation:
//! - Per-pass shards for failure isolation
//! - Emergency shards for failover (no allocation under pressure)
//! - Tenant shards for cryptographic isolation
//!
//! Sharding ensures that a bug in one pass cannot corrupt another pass's state,
//! and that memory leaks are isolated to individual shards.

pub mod shard;
pub mod pool;
pub mod emergency;
pub mod tenant;

pub use shard::{PassShard, ShardLocalLog};
pub use pool::{HardwarePool, HardwareAllocation, CircuitBreaker, CircuitState};
pub use emergency::{EmergencyShardPool, EmergencyShard};
pub use tenant::{TenantShard, ProtectedMemoryRegion};

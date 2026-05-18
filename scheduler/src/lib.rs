//! Scheduler with Dominant Resource Fairness and reputation-weighted history
//!
//! This implements Problem 3: Fair scheduling with adversarial tenants.
//! 
//! Key concepts:
//! - Dominant Resource Fairness (DRF): Fairness over each tenant's dominant resource
//! - Reputation weighting: Tenants who use what they're allocated get priority
//! - Bi-temporal logging: Provable allocation and utilization tracking
//!
//! This prevents gaming because submitting more requests doesn't shift which
//! resource is dominant, and reputation decreases for wasteful tenants.

pub mod drf;
pub mod reputation;
pub mod schedule;
pub mod optimization;

pub use drf::{ResourceType, ResourceShare, DominantResourceFairness};
pub use reputation::{TenantReputation, ReputationTracker};
pub use schedule::{Schedule, ScheduledPass, PassRequest};
pub use optimization::{ScheduleOptimizer, OptimizationConfig};

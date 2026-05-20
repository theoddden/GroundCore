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
pub mod keyhole;
pub mod optimization;
pub mod reputation;
pub mod schedule;

pub use drf::{DominantResourceFairness, ResourceShare, ResourceType};
pub use keyhole::{
    GroundStationGeometry, KeyholeConfig, KeyholeWarning, MountType, analyse_pass,
    is_likely_keyhole_pass, keyhole_score_penalty,
};
pub use optimization::{OptimizationConfig, ScheduleOptimizer};
pub use reputation::{ReputationTracker, TenantReputation};
pub use schedule::{PassRequest, Schedule, ScheduledPass};

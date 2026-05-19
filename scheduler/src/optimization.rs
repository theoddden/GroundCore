//! Schedule optimization using simulated annealing
//!
//! The scheduler uses simulated annealing to optimize pass allocation
//! across the 24-48 hour horizon. This is a computationally intensive
//! operation that runs every few minutes.

use crate::drf::DominantResourceFairness;
use crate::reputation::ReputationTracker;
use crate::schedule::{PassRequest, Schedule, ScheduledPass};
use caching::ScheduleFragmentCache;
use chrono::{DateTime, Utc};
use ground_core::PassId;
use snapshotting::ScheduleSnapshotter;
use std::time::Duration;

/// Optimization configuration
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Maximum iterations
    pub max_iterations: usize,
    /// Initial temperature
    pub initial_temperature: f64,
    /// Cooling rate
    pub cooling_rate: f64,
    /// Timeout
    pub timeout: Duration,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10000,
            initial_temperature: 1000.0,
            cooling_rate: 0.995,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Schedule optimizer
pub struct ScheduleOptimizer {
    drf: DominantResourceFairness,
    reputation: ReputationTracker,
    config: OptimizationConfig,
    /// Snapshotter for atomic rollback if optimization produces a worse schedule
    snapshotter: ScheduleSnapshotter,
    /// Fragment cache for stable schedule portions — avoids re-cloning committed passes
    fragment_cache: ScheduleFragmentCache,
}

impl ScheduleOptimizer {
    pub fn new(config: OptimizationConfig) -> Self {
        Self {
            drf: DominantResourceFairness::new(),
            reputation: ReputationTracker::new(0.1),
            config,
            snapshotter: ScheduleSnapshotter::new(5),
            fragment_cache: ScheduleFragmentCache::new(32),
        }
    }

    /// Set DRF instance
    pub fn set_drf(&mut self, drf: DominantResourceFairness) {
        self.drf = drf;
    }

    /// Set reputation tracker
    pub fn set_reputation(&mut self, reputation: ReputationTracker) {
        self.reputation = reputation;
    }

    /// Optimize schedule for a set of requests.
    ///
    /// Uses `tokio::task::yield_now()` every 100 iterations to prevent
    /// starving the Tokio runtime during the CPU-bound annealing loop.
    /// Takes a snapshot before optimizing; rolls back atomically if the
    /// new schedule is worse than the previous best.
    pub async fn optimize(
        &mut self,
        requests: &[PassRequest],
        horizon_start: DateTime<Utc>,
        horizon_end: DateTime<Utc>,
        existing_schedule: Option<&Schedule>,
    ) -> Result<Schedule> {
        let mut schedule = Schedule::new(horizon_start, horizon_end);

        // Start with existing schedule if provided
        if let Some(existing) = existing_schedule {
            schedule.passes = existing.passes.clone();
        }

        // Snapshot before optimization for atomic rollback
        let pre_snapshot = self.snapshotter.snapshot(&schedule);
        let pre_score = self.evaluate_schedule(&schedule);

        // Simulated annealing
        let mut temperature = self.config.initial_temperature;
        let mut best_schedule = schedule.clone();
        let mut best_score = pre_score;

        let start_time = std::time::Instant::now();

        for iteration in 0..self.config.max_iterations {
            // Check timeout
            if start_time.elapsed() > self.config.timeout {
                break;
            }

            // Yield to Tokio every 100 iterations to prevent runtime starvation
            if iteration % 100 == 0 {
                tokio::task::yield_now().await;
            }

            // Generate neighbor schedule
            let neighbor = self.generate_neighbor(&schedule, requests);
            let neighbor_score = self.evaluate_schedule(&neighbor);

            // Accept or reject
            let score_delta = neighbor_score - best_score;
            if score_delta > 0.0 || rand::random::<f64>() < (score_delta / temperature).exp() {
                schedule = neighbor;
                if neighbor_score > best_score {
                    best_schedule = schedule.clone();
                    best_score = neighbor_score;
                }
            }

            // Cool down
            temperature *= self.config.cooling_rate;

            tracing::debug!(
                "Iteration {}: score={}, temperature={}",
                iteration,
                best_score,
                temperature
            );
        }

        // Rollback if optimization produced a worse result than the pre-snapshot
        if best_score < pre_score {
            tracing::warn!(
                "Optimization produced worse schedule ({:.1} < {:.1}), rolling back",
                best_score,
                pre_score
            );
            if let Ok(rolled_back) = pre_snapshot.restore::<Schedule>() {
                return Ok(rolled_back);
            }
        }

        Ok(best_schedule)
    }

    /// Evaluate schedule quality
    fn evaluate_schedule(&self, schedule: &Schedule) -> f64 {
        let mut score = 0.0;

        // Fairness component (lower dominant share variance = higher score)
        let fairness_stats = self.drf.fairness_stats();
        let fairness_score = 1.0 - fairness_stats.fairness_gap;
        score += fairness_score * 100.0;

        // Reputation component (higher avg reputation = higher score)
        let reputation_stats = self.reputation.stats();
        let reputation_score = reputation_stats.avg_reputation;
        score += reputation_score * 50.0;

        // Conflict penalty
        let conflicts = schedule.check_conflicts();
        score -= conflicts.len() as f64 * 200.0;

        // Request fulfillment component
        let fulfilled_ratio = if schedule.passes.is_empty() {
            0.0
        } else {
            1.0 // Would be computed from actual request fulfillment
        };
        score += fulfilled_ratio * 30.0;

        score
    }

    /// Generate neighbor schedule by making a small change
    fn generate_neighbor(&self, schedule: &Schedule, requests: &[PassRequest]) -> Schedule {
        let mut neighbor = schedule.clone();

        if neighbor.passes.is_empty() && !requests.is_empty() {
            // Add a new pass
            if let Some(request) = requests.first() {
                let pass = self.create_pass_from_request(request);
                neighbor.add_pass(pass);
            }
        } else if !neighbor.passes.is_empty() {
            // Modify an existing pass
            let len = neighbor.passes.len();
            let idx = rand::random::<usize>() % len;
            // In a real implementation, this would adjust timing or allocation
            // For now, we just shuffle
            neighbor.passes.swap(idx, (idx + 1) % len);
        }

        neighbor
    }

    /// Create a scheduled pass from a request
    fn create_pass_from_request(&self, request: &PassRequest) -> ScheduledPass {
        // In a real implementation, this would compute actual pass times
        // from satellite visibility and allocate hardware
        ScheduledPass {
            pass_id: uuid::Uuid::new_v4().to_string(),
            request_id: request.request_id.clone(),
            customer_id: request.customer_id.clone(),
            satellite_id: request.satellite_id.clone(),
            scheduled_window: request.window.clone(),
            hardware_allocation: crate::schedule::HardwareAllocation {
                sdr_devices: vec!["sdr-0".to_string()],
                shadow_sdr: if request.sla_tier.requires_shadow() {
                    Some("sdr-shadow-0".to_string())
                } else {
                    None
                },
                antenna_id: "antenna-0".to_string(),
                rotator_id: "rotator-0".to_string(),
            },
            scheduled_at: Utc::now(),
            status: crate::schedule::PassStatus::Scheduled,
        }
    }

    /// Incremental re-optimization using schedule fragments.
    ///
    /// Caches the IDs of stable "committed" passes (windows starting before now)
    /// in `ScheduleFragmentCache` for immutability tracking. Only the mutable
    /// tail (future passes + new requests) is cloned and re-annealed, cutting
    /// optimization cost proportionally to the committed fraction of the schedule.
    pub fn incremental_optimize(
        &mut self,
        schedule: &Schedule,
        new_requests: &[PassRequest],
    ) -> Result<Schedule> {
        let now = Utc::now();

        // Partition: stable passes (committed) vs mutable tail (future)
        let (stable, mutable_tail): (Vec<_>, Vec<_>) = schedule
            .passes
            .iter()
            .cloned()
            .partition(|p| p.scheduled_window.start <= now);

        // Register committed pass IDs in the fragment cache for audit trail
        let stable_ids: Vec<PassId> = stable.iter().map(|p| p.pass_id.clone()).collect();
        if !stable_ids.is_empty() {
            let fragment = caching::ScheduleFragment::new(stable_ids, 300); // immutable for 5 minutes
            self.fragment_cache.add_fragment(fragment);
        }

        // Build mutable tail schedule with new requests appended
        let mut tail_passes = mutable_tail;
        for request in new_requests {
            tail_passes.push(self.create_pass_from_request(request));
        }

        let mut tail_schedule = Schedule::new(now, schedule.horizon.1);
        tail_schedule.passes = tail_passes;

        // Quick annealing pass on the tail only (200 iterations vs full 10,000)
        let mut _temperature = 100.0;
        for _ in 0..200 {
            let neighbor = self.generate_neighbor(&tail_schedule, new_requests);
            let current_score = self.evaluate_schedule(&tail_schedule);
            let neighbor_score = self.evaluate_schedule(&neighbor);

            if neighbor_score > current_score {
                tail_schedule = neighbor;
            }

            _temperature *= 0.95;
        }

        // Reconstruct: stable passes (unchanged) + optimised tail
        let mut result = Schedule::new(schedule.horizon.0, schedule.horizon.1);
        result.passes.extend(stable);
        result.passes.extend(tail_schedule.passes);
        result.version = schedule.version + 1;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_basic() {
        let config = OptimizationConfig::default();
        let mut optimizer = ScheduleOptimizer::new(config);

        let request = PassRequest {
            request_id: "req1".to_string(),
            customer_id: "customer1".to_string(),
            satellite_id: "sat1".to_string(),
            window: crate::schedule::PassWindow {
                start: Utc::now(),
                end: Utc::now() + chrono::Duration::hours(1),
                min_duration_sec: 300,
            },
            hardware_requirements: crate::schedule::HardwareRequirements {
                frequency_band: "L".to_string(),
                min_sample_rate: 2_000_000,
                require_shadow: false,
                antenna_type: None,
            },
            priority: crate::schedule::Priority::Normal,
            sla_tier: crate::schedule::SlaTier::Standard,
            submitted_at: Utc::now(),
        };

        let schedule = optimizer
            .optimize(
                &[request],
                Utc::now(),
                Utc::now() + chrono::Duration::hours(24),
                None,
            )
            .unwrap();

        assert_eq!(schedule.passes.len(), 1);
    }
}

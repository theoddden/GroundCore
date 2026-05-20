// Satellite Scheduler - Kubernetes-like scheduling across satellite nodes

use crate::resource::{ResourceAllocation, ResourceClaim, SatelliteNode};
use crate::{AllocationId, PreemptionReason, Priority, SatelliteId, TaskId, TenantId, TimeWindow};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Satellite scheduler trait
pub trait SatelliteScheduler: Send + Sync {
    /// Allocate resources for a claim
    fn allocate(&mut self, claim: ResourceClaim) -> Result<AllocationId, ScheduleError>;

    /// Preempt an allocation
    fn preempt(
        &mut self,
        allocation_id: AllocationId,
        reason: PreemptionReason,
    ) -> Result<(), ScheduleError>;

    /// Rebalance allocations across satellites
    fn rebalance(&mut self) -> RebalanceReport;

    /// Forecast availability over time window
    fn forecast_availability(&self, window: &TimeWindow) -> AvailabilityForecast;
}

/// Default satellite scheduler implementation
pub struct DefaultSatelliteScheduler {
    satellites: HashMap<SatelliteId, SatelliteNode>,
    allocator: crate::resource::ResourceAllocator,
}

impl DefaultSatelliteScheduler {
    pub fn new() -> Self {
        Self {
            satellites: HashMap::new(),
            allocator: crate::resource::ResourceAllocator::default(),
        }
    }

    pub fn add_satellite(&mut self, satellite: SatelliteNode) {
        self.satellites
            .insert(satellite.satellite_id.clone(), satellite);
    }

    fn find_best_satellite(&self, claim: &ResourceClaim) -> Option<&SatelliteNode> {
        self.satellites
            .values()
            .filter(|sat| sat.can_satisfy(claim))
            .min_by(|a, b| {
                // Prefer satellite with more available resources
                let a_avail = a.power_budget.available_watts;
                let b_avail = b.power_budget.available_watts;
                b_avail
                    .partial_cmp(&a_avail)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

impl SatelliteScheduler for DefaultSatelliteScheduler {
    fn allocate(&mut self, claim: ResourceClaim) -> Result<AllocationId, ScheduleError> {
        let satellite = self
            .find_best_satellite(&claim)
            .ok_or_else(|| ScheduleError::NoAvailableNode)?;

        let task_id = TaskId::new_v4();
        let tenant_id = TenantId::from("default");
        let valid_window = TimeWindow::new(Utc::now(), Utc::now() + claim.duration);
        let priority = Priority::Medium;

        // Clone satellite_id before mutable borrow
        let sat_id = satellite.satellite_id.clone();

        let allocation_id = self.allocator.allocate(
            claim.clone(),
            task_id,
            &tenant_id,
            priority,
            &valid_window,
        )
        .map_err(|e| ScheduleError::AllocationFailed(e.to_string()))?;

        // Update satellite state
        if let Some(sat) = self.satellites.get_mut(&sat_id) {
            sat.allocate(claim, task_id, tenant_id.clone());
        }

        Ok(allocation_id)
    }

    fn preempt(
        &mut self,
        allocation_id: AllocationId,
        reason: PreemptionReason,
    ) -> Result<(), ScheduleError> {
        self.allocator
            .preempt(&allocation_id, reason)
            .map_err(|e| ScheduleError::PreemptionFailed(e.to_string()))?;

        // Release from satellite
        for satellite in self.satellites.values_mut() {
            satellite.release(&allocation_id);
        }

        Ok(())
    }

    fn rebalance(&mut self) -> RebalanceReport {
        let mut moved_allocations = 0;
        let mut failed_migrations = 0;

        // Simple rebalancing: move allocations from overloaded to underloaded satellites
        let mut allocations_to_move = Vec::new();

        for (sat_id, satellite) in &self.satellites {
            let utilization =
                satellite.power_budget.allocated_watts / satellite.power_budget.total_watts;
            if utilization > 0.8 {
                // Find allocations that can be moved
                for allocation in &satellite.current_allocations {
                    if allocation.preemptible && allocation.priority != Priority::Critical {
                        allocations_to_move.push((sat_id.clone(), allocation.clone()));
                    }
                }
            }
        }

        for (source_sat, allocation) in allocations_to_move {
            // Try to find a less loaded satellite
            let claim = allocation.resources.clone();
            if let Some(target_sat) = self.find_best_satellite(&claim) {
                if target_sat.satellite_id != source_sat {
                    // Move allocation
                    self.allocator.release(&allocation.allocation_id).ok();

                    let task_id = TaskId::new_v4();
                    let tenant_id = allocation.tenant_id.clone();
                    let valid_window = allocation.valid_window.clone();
                    let priority = allocation.priority;

                    if self
                        .allocator
                        .allocate(claim, task_id, tenant_id, priority, valid_window)
                        .is_ok()
                    {
                        moved_allocations += 1;
                    } else {
                        failed_migrations += 1;
                    }
                }
            }
        }

        RebalanceReport {
            allocations_moved: moved_allocations,
            failed_migrations,
            timestamp: Utc::now(),
        }
    }

    fn forecast_availability(&self, window: &TimeWindow) -> AvailabilityForecast {
        let mut satellite_availability = HashMap::new();

        for (sat_id, satellite) in &self.satellites {
            let forecast = SatelliteAvailability {
                satellite_id: sat_id.clone(),
                available_power: satellite.power_budget.available_watts,
                available_bandwidth: satellite.available_bandwidth(),
                available_terminals: satellite
                    .optical_terminals
                    .values()
                    .filter(|t| t.available)
                    .map(|t| t.terminal_id.clone())
                    .collect(),
                confidence: crate::ConfidenceScore::new(0.9),
            };
            satellite_availability.insert(sat_id.clone(), forecast);
        }

        AvailabilityForecast {
            window: window.clone(),
            satellite_availability,
        }
    }
}

impl Default for DefaultSatelliteScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Rebalance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceReport {
    pub allocations_moved: usize,
    pub failed_migrations: usize,
    pub timestamp: DateTime<Utc>,
}

/// Availability forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityForecast {
    pub window: TimeWindow,
    pub satellite_availability: HashMap<SatelliteId, SatelliteAvailability>,
}

/// Satellite availability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteAvailability {
    pub satellite_id: SatelliteId,
    pub available_power: f64,
    pub available_bandwidth: crate::DataRate,
    pub available_terminals: Vec<crate::TerminalId>,
    pub confidence: crate::ConfidenceScore,
}

/// Schedule error
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScheduleError {
    #[error("No available satellite node for allocation")]
    NoAvailableNode,

    #[error("Allocation failed: {0}")]
    AllocationFailed(String),

    #[error("Preemption failed: {0}")]
    PreemptionFailed(String),

    #[error("Satellite not found: {0}")]
    SatelliteNotFound(String),
}

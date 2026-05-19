// Resource Allocator - multi-tenant resource allocation

use crate::{
    AllocationId, Bytes, DataRate, Priority, SatelliteId, TaskId, TenantId, TerminalId, TimeWindow,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Resource claim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceClaim {
    pub optical_terminal: Option<TerminalId>,
    pub bandwidth: Option<DataRate>,
    pub duration: Duration,
    pub power_watts: f64,
    pub storage_bytes: Option<u64>,
    pub compute_cycles: Option<u64>, // For on-orbit compute
}

/// Resource allocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub allocation_id: AllocationId,
    pub task_id: TaskId,
    pub tenant_id: TenantId, // Multi-tenant from day one
    pub resources: ResourceClaim,
    pub valid_window: TimeWindow,
    pub priority: Priority,
    pub preemptible: bool,
}

impl ResourceAllocation {
    pub fn new(
        task_id: TaskId,
        tenant_id: TenantId,
        resources: ResourceClaim,
        valid_window: TimeWindow,
        priority: Priority,
        preemptible: bool,
    ) -> Self {
        Self {
            allocation_id: AllocationId::new_v4(),
            task_id,
            tenant_id,
            resources,
            valid_window,
            priority,
            preemptible,
        }
    }

    /// Check if allocation is currently valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        self.valid_window.contains(now)
    }

    /// Check if allocation can be preempted
    pub fn can_preempt(&self) -> bool {
        self.preemptible && self.priority != Priority::Critical
    }
}

/// Resource allocator
///
/// Uses a `terminal_index` to map each optical terminal ID to the set of
/// allocations currently using it. This makes `check_conflicts` O(k) where
/// k is the number of overlapping allocations on that terminal, instead of
/// O(n) over all allocations.
pub struct ResourceAllocator {
    allocations: Vec<ResourceAllocation>,
    tenant_quotas: HashMap<TenantId, TenantQuota>,
    /// Index: terminal_id -> vec of allocation_ids using that terminal
    terminal_index: HashMap<TerminalId, Vec<AllocationId>>,
    /// Index: tenant_id -> vec of allocation_ids owned by that tenant
    tenant_index: HashMap<TenantId, Vec<AllocationId>>,
}

impl ResourceAllocator {
    pub fn new() -> Self {
        Self {
            allocations: Vec::new(),
            tenant_quotas: HashMap::new(),
            terminal_index: HashMap::new(),
            tenant_index: HashMap::new(),
        }
    }

    /// Set tenant quota
    pub fn set_tenant_quota(&mut self, tenant_id: TenantId, quota: TenantQuota) {
        self.tenant_quotas.insert(tenant_id, quota);
    }

    /// Attempt to allocate resources
    pub fn allocate(
        &mut self,
        claim: ResourceClaim,
        task_id: TaskId,
        tenant_id: TenantId,
        priority: Priority,
        valid_window: TimeWindow,
    ) -> Result<AllocationId, AllocationError> {
        // Check tenant quota
        if let Some(quota) = self.tenant_quotas.get(&tenant_id) {
            let current_usage = self.tenant_usage(&tenant_id);
            if current_usage + claim.power_watts > quota.max_power_watts {
                return Err(AllocationError::QuotaExceeded {
                    tenant_id,
                    resource: "power".to_string(),
                });
            }
        }

        // Check for conflicts with existing allocations
        if let Some(conflict) = self.check_conflicts(&claim, &valid_window, &tenant_id) {
            return Err(AllocationError::ResourceConflict {
                conflicting_allocation: conflict,
            });
        }

        let allocation = ResourceAllocation::new(
            task_id,
            tenant_id,
            claim,
            valid_window,
            priority,
            true, // Default to preemptible unless critical
        );

        let allocation_id = allocation.allocation_id;

        // Update terminal index
        if let Some(terminal) = &allocation.resources.optical_terminal {
            self.terminal_index
                .entry(terminal.clone())
                .or_default()
                .push(allocation_id);
        }
        // Update tenant index
        self.tenant_index
            .entry(allocation.tenant_id.clone())
            .or_default()
            .push(allocation_id);

        self.allocations.push(allocation);
        Ok(allocation_id)
    }

    /// Release allocation
    pub fn release(&mut self, allocation_id: &AllocationId) -> Result<(), AllocationError> {
        let pos = self
            .allocations
            .iter()
            .position(|a| &a.allocation_id == allocation_id)
            .ok_or_else(|| AllocationError::NotFound {
                allocation_id: *allocation_id,
            })?;

        let removed = self.allocations.remove(pos);

        // Clean up terminal index
        if let Some(terminal) = &removed.resources.optical_terminal {
            if let Some(ids) = self.terminal_index.get_mut(terminal) {
                ids.retain(|id| id != allocation_id);
            }
        }
        // Clean up tenant index
        if let Some(ids) = self.tenant_index.get_mut(&removed.tenant_id) {
            ids.retain(|id| id != allocation_id);
        }

        Ok(())
    }

    /// Preempt allocation (for higher priority tasks)
    pub fn preempt(
        &mut self,
        allocation_id: &AllocationId,
        reason: PreemptionReason,
    ) -> Result<(), AllocationError> {
        let allocation = self
            .allocations
            .iter()
            .find(|a| &a.allocation_id == allocation_id)
            .ok_or_else(|| AllocationError::NotFound {
                allocation_id: *allocation_id,
            })?;

        if !allocation.can_preempt() {
            return Err(AllocationError::NotPreemptible {
                allocation_id: *allocation_id,
            });
        }

        self.release(allocation_id)?;
        tracing::info!(
            "Preempted allocation {:?} due to: {:?}",
            allocation_id,
            reason
        );
        Ok(())
    }

    /// O(k) conflict check via terminal index, where k = allocations on that terminal.
    fn check_conflicts(
        &self,
        claim: &ResourceClaim,
        window: &TimeWindow,
        tenant_id: &TenantId,
    ) -> Option<AllocationId> {
        if let Some(terminal) = &claim.optical_terminal {
            if let Some(ids) = self.terminal_index.get(terminal) {
                for alloc_id in ids {
                    if let Some(allocation) = self
                        .allocations
                        .iter()
                        .find(|a| &a.allocation_id == alloc_id)
                    {
                        if allocation.valid_window.overlaps(window) {
                            return Some(allocation.allocation_id);
                        }
                    }
                }
            }
        }
        None
    }

    /// O(k) tenant usage via tenant index, where k = allocations owned by tenant.
    fn tenant_usage(&self, tenant_id: &TenantId) -> f64 {
        self.tenant_index
            .get(tenant_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.allocations.iter().find(|a| &a.allocation_id == id))
                    .map(|a| a.resources.power_watts)
                    .sum()
            })
            .unwrap_or(0.0)
    }

    /// Get allocations for a tenant
    pub fn get_tenant_allocations(&self, tenant_id: &TenantId) -> Vec<&ResourceAllocation> {
        self.allocations
            .iter()
            .filter(|a| &a.tenant_id == tenant_id)
            .collect()
    }

    /// Get all allocations
    pub fn get_all_allocations(&self) -> &[ResourceAllocation] {
        &self.allocations
    }
}

impl Default for ResourceAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Tenant quota
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuota {
    pub max_power_watts: f64,
    pub max_bandwidth_bps: u64,
    pub max_storage_bytes: u64,
    pub max_allocations: usize,
}

/// Allocation error
#[derive(Debug, Clone, thiserror::Error)]
pub enum AllocationError {
    #[error("Allocation not found: {allocation_id:?}")]
    NotFound { allocation_id: AllocationId },

    #[error("Tenant quota exceeded for {tenant_id}: resource {resource}")]
    QuotaExceeded {
        tenant_id: TenantId,
        resource: String,
    },

    #[error("Resource conflict with allocation {conflicting_allocation:?}")]
    ResourceConflict {
        conflicting_allocation: AllocationId,
    },

    #[error("Allocation {allocation_id:?} is not preemptible")]
    NotPreemptible { allocation_id: AllocationId },

    #[error("Insufficient resources: {0}")]
    InsufficientResources(String),
}

// Re-export PreemptionReason from crate root
pub use crate::PreemptionReason;

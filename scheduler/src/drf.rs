//! Dominant Resource Fairness implementation
//!
//! DRF ensures fairness over each tenant's dominant resource rather than
//! a single shared resource. This prevents gaming because tenants can't
//! shift their dominant resource by submitting more requests.

use ground_core::CustomerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Resource types that can be constrained
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// L-band SDR time
    LBandSdrTime,
    /// S-band SDR time
    SBandSdrTime,
    /// Rotator hours
    RotatorHours,
    /// Data egress bandwidth
    EgressBandwidth,
    /// Total passes
    TotalPasses,
}

/// Share of a specific resource
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResourceShare {
    /// Resource type
    pub resource_type: ResourceType,
    /// Total capacity of this resource
    pub total_capacity: f64,
    /// Amount allocated to this tenant
    pub allocated: f64,
    /// Amount actually used by this tenant
    pub used: f64,
}

impl ResourceShare {
    pub fn new(resource_type: ResourceType, total_capacity: f64) -> Self {
        Self {
            resource_type,
            total_capacity,
            allocated: 0.0,
            used: 0.0,
        }
    }

    /// Get the share of total capacity (0.0 to 1.0)
    pub fn share(&self) -> f64 {
        if self.total_capacity > 0.0 {
            self.allocated / self.total_capacity
        } else {
            0.0
        }
    }

    /// Get utilization rate (used / allocated)
    pub fn utilization(&self) -> f64 {
        if self.allocated > 0.0 {
            self.used / self.allocated
        } else {
            0.0
        }
    }
}

/// Resource allocation for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// Shares for each resource type
    pub shares: HashMap<ResourceType, ResourceShare>,
    /// Dominant resource type (the resource with highest share)
    pub dominant_resource: Option<ResourceType>,
}

impl ResourceAllocation {
    pub fn new() -> Self {
        Self {
            shares: HashMap::new(),
            dominant_resource: None,
        }
    }
}

impl Default for ResourceAllocation {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceAllocation {
    /// Add a resource share
    pub fn add_share(&mut self, share: ResourceShare) {
        self.shares.insert(share.resource_type, share);
        self.recompute_dominant();
    }

    /// Get share for a specific resource
    pub fn get_share(&self, resource_type: ResourceType) -> Option<&ResourceShare> {
        self.shares.get(&resource_type)
    }

    /// Update allocated amount for a resource
    pub fn allocate(&mut self, resource_type: ResourceType, amount: f64) {
        if let Some(share) = self.shares.get_mut(&resource_type) {
            share.allocated += amount;
            self.recompute_dominant();
        }
    }

    /// Update used amount for a resource
    pub fn use_resource(&mut self, resource_type: ResourceType, amount: f64) {
        if let Some(share) = self.shares.get_mut(&resource_type) {
            share.used += amount;
        }
    }

    /// Recpute dominant resource
    fn recompute_dominant(&mut self) {
        let dominant = self
            .shares
            .iter()
            .max_by(|a, b| a.1.share().partial_cmp(&b.1.share()).unwrap())
            .map(|(rt, _)| *rt);

        self.dominant_resource = dominant;
    }

    /// Get the dominant share
    pub fn dominant_share(&self) -> f64 {
        if let Some(dominant) = self.dominant_resource {
            self.shares.get(&dominant).map(|s| s.share()).unwrap_or(0.0)
        } else {
            0.0
        }
    }
}

/// Dominant Resource Fairness scheduler
pub struct DominantResourceFairness {
    /// Tenant allocations
    allocations: HashMap<CustomerId, ResourceAllocation>,
    /// Total system capacity per resource
    total_capacity: HashMap<ResourceType, f64>,
}

impl DominantResourceFairness {
    pub fn new() -> Self {
        Self {
            allocations: HashMap::new(),
            total_capacity: HashMap::new(),
        }
    }
}

impl Default for DominantResourceFairness {
    fn default() -> Self {
        Self::new()
    }
}

impl DominantResourceFairness {
    /// Set total capacity for a resource
    pub fn set_capacity(&mut self, resource_type: ResourceType, capacity: f64) {
        self.total_capacity.insert(resource_type, capacity);
    }

    /// Get or create tenant allocation
    pub fn get_or_create_allocation(&mut self, tenant_id: CustomerId) -> &mut ResourceAllocation {
        self.allocations.entry(tenant_id).or_default()
    }

    /// Get tenant allocation
    pub fn get_allocation(&self, tenant_id: &CustomerId) -> Option<&ResourceAllocation> {
        self.allocations.get(tenant_id)
    }

    /// Compute scheduling priority for a tenant
    /// Lower value = higher priority (DRF minimizes maximum dominant share)
    pub fn scheduling_priority(&self, tenant_id: &CustomerId) -> f64 {
        if let Some(allocation) = self.get_allocation(tenant_id) {
            allocation.dominant_share()
        } else {
            0.0 // New tenant has zero share, highest priority
        }
    }

    /// Find the tenant with highest scheduling priority
    pub fn select_next_tenant(&self, candidates: &[CustomerId]) -> Option<CustomerId> {
        candidates
            .iter()
            .min_by(|a, b| {
                let priority_a = self.scheduling_priority(a);
                let priority_b = self.scheduling_priority(b);
                priority_a.partial_cmp(&priority_b).unwrap()
            })
            .cloned()
    }

    /// Check if allocating resources would violate fairness
    pub fn can_allocate(
        &self,
        tenant_id: &CustomerId,
        resource_type: ResourceType,
        amount: f64,
        fairness_threshold: f64,
    ) -> bool {
        if let Some(allocation) = self.get_allocation(tenant_id) {
            let _current_share = allocation.dominant_share();
            let total_cap = self
                .total_capacity
                .get(&resource_type)
                .copied()
                .unwrap_or(0.0);

            if total_cap > 0.0 {
                let new_allocated = allocation
                    .get_share(resource_type)
                    .map(|s| s.allocated + amount)
                    .unwrap_or(amount);
                let new_share = new_allocated / total_cap;

                // Would this exceed fairness threshold?
                new_share <= fairness_threshold
            } else {
                false
            }
        } else {
            true // New tenant can always allocate
        }
    }

    /// Get fairness statistics
    pub fn fairness_stats(&self) -> FairnessStats {
        let num_tenants = self.allocations.len();
        let dominant_shares: Vec<f64> = self
            .allocations
            .values()
            .map(|a| a.dominant_share())
            .collect();

        let max_share = dominant_shares.iter().cloned().fold(0.0_f64, f64::max);
        let min_share = dominant_shares.iter().cloned().fold(1.0_f64, f64::min);
        let avg_share = if num_tenants > 0 {
            dominant_shares.iter().sum::<f64>() / num_tenants as f64
        } else {
            0.0
        };

        FairnessStats {
            num_tenants,
            max_dominant_share: max_share,
            min_dominant_share: min_share,
            avg_dominant_share: avg_share,
            fairness_gap: max_share - min_share,
        }
    }
}

/// Fairness statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairnessStats {
    pub num_tenants: usize,
    pub max_dominant_share: f64,
    pub min_dominant_share: f64,
    pub avg_dominant_share: f64,
    pub fairness_gap: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drf_basic() {
        let mut drf = DominantResourceFairness::new();
        drf.set_capacity(ResourceType::LBandSdrTime, 100.0);
        drf.set_capacity(ResourceType::RotatorHours, 50.0);

        let tenant1 = "tenant1".to_string();
        let tenant2 = "tenant2".to_string();

        let alloc1 = drf.get_or_create_allocation(tenant1.clone());
        alloc1.add_share(ResourceShare::new(ResourceType::LBandSdrTime, 100.0));
        alloc1.allocate(ResourceType::LBandSdrTime, 30.0);

        let alloc2 = drf.get_or_create_allocation(tenant2.clone());
        alloc2.add_share(ResourceShare::new(ResourceType::LBandSdrTime, 100.0));
        alloc2.allocate(ResourceType::LBandSdrTime, 10.0);

        // Tenant2 should have higher priority (lower share)
        assert!(drf.scheduling_priority(&tenant2) < drf.scheduling_priority(&tenant1));

        let selected = drf.select_next_tenant(&[tenant1.clone(), tenant2.clone()]);
        assert_eq!(selected, Some(tenant2));
    }

    #[test]
    fn test_dominant_resource_detection() {
        let mut alloc = ResourceAllocation::new();
        alloc.add_share(ResourceShare::new(ResourceType::LBandSdrTime, 100.0));
        alloc.add_share(ResourceShare::new(ResourceType::RotatorHours, 50.0));

        alloc.allocate(ResourceType::LBandSdrTime, 40.0); // 40% of L-band
        alloc.allocate(ResourceType::RotatorHours, 30.0); // 60% of rotator (dominant)

        assert_eq!(alloc.dominant_resource, Some(ResourceType::RotatorHours));
        assert!((alloc.dominant_share() - 0.6).abs() < 0.01);
    }
}

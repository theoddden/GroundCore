//! Reputation tracking for tenants
//!
//! Reputation weighting ensures tenants who use what they're allocated
//! get scheduling priority over tenants who routinely waste capacity.

use ground_core::CustomerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tenant scheduling state with reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantReputation {
    /// Customer ID
    pub tenant_id: CustomerId,
    /// Dominant resource type
    pub dominant_resource: Option<String>,
    /// Historical utilization (exponential moving average)
    pub historical_utilization: f64,
    /// Reputation score (0.0 to 1.0)
    pub reputation: f64,
    /// Current allocation
    pub current_allocation: ResourceUsage,
    /// Total requests submitted
    pub total_requests: u64,
    /// Total requests fulfilled
    pub total_fulfilled: u64,
    /// Total wasted allocations (allocated but not used)
    pub total_wasted: f64,
}

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub allocated: f64,
    pub used: f64,
}

impl ResourceUsage {
    pub fn new() -> Self {
        Self {
            allocated: 0.0,
            used: 0.0,
        }
    }
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceUsage {
    pub fn utilization_rate(&self) -> f64 {
        if self.allocated > 0.0 {
            self.used / self.allocated
        } else {
            0.0
        }
    }
}

impl TenantReputation {
    pub fn new(tenant_id: CustomerId) -> Self {
        Self {
            tenant_id,
            dominant_resource: None,
            historical_utilization: 0.5, // Start neutral
            reputation: 1.0,             // Start with perfect reputation
            current_allocation: ResourceUsage {
                allocated: 100.0,
                used: 0.0,
            },
            total_requests: 0,
            total_fulfilled: 0,
            total_wasted: 0.0,
        }
    }

    /// Update reputation based on utilization
    pub fn update_utilization(&mut self, utilization: f64) {
        // Exponential moving average with alpha = 0.1
        self.historical_utilization = 0.1 * utilization + 0.9 * self.historical_utilization;

        // Reputation follows utilization with some hysteresis
        if self.historical_utilization > 0.8 {
            // High utilization: increase reputation
            self.reputation = (self.reputation * 0.9 + 1.0 * 0.1).min(1.0);
        } else if self.historical_utilization < 0.5 {
            // Low utilization: decrease reputation
            self.reputation = (self.reputation * 0.9 + 0.5 * 0.1).max(0.0);
        }
    }

    /// Record a request
    pub fn record_request(&mut self) {
        self.total_requests += 1;
    }

    /// Record a fulfilled request
    pub fn record_fulfilled(&mut self, allocated: f64, used: f64) {
        self.total_fulfilled += 1;
        self.current_allocation.allocated += allocated;
        self.current_allocation.used += used;

        // Track waste
        let waste = (allocated - used).max(0.0);
        self.total_wasted += waste;

        // Update utilization
        let utilization = if allocated > 0.0 {
            used / allocated
        } else {
            0.0
        };
        self.update_utilization(utilization);
    }

    /// Compute scheduling priority with reputation weighting
    pub fn scheduling_priority(&self) -> f64 {
        // Base priority from DRF (lower share = higher priority)
        let base = 1.0 / (self.current_allocation.allocated + 1.0);

        // Apply reputation weighting
        base * self.reputation
    }

    /// Get waste ratio
    pub fn waste_ratio(&self) -> f64 {
        if self.current_allocation.allocated > 0.0 {
            self.total_wasted / self.current_allocation.allocated
        } else {
            0.0
        }
    }
}

/// Reputation tracker for all tenants
pub struct ReputationTracker {
    tenants: HashMap<CustomerId, TenantReputation>,
    #[allow(dead_code)]
    alpha: f64, // EMA smoothing factor
}

impl ReputationTracker {
    pub fn new(alpha: f64) -> Self {
        Self {
            tenants: HashMap::new(),
            alpha,
        }
    }

    /// Get or create tenant reputation
    pub fn get_or_create(&mut self, tenant_id: CustomerId) -> &mut TenantReputation {
        self.tenants
            .entry(tenant_id.clone())
            .or_insert_with(|| TenantReputation::new(tenant_id))
    }

    /// Get tenant reputation
    pub fn get(&self, tenant_id: &CustomerId) -> Option<&TenantReputation> {
        self.tenants.get(tenant_id)
    }

    /// Update tenant utilization
    pub fn update_utilization(&mut self, tenant_id: &CustomerId, utilization: f64) {
        if let Some(reputation) = self.tenants.get_mut(tenant_id) {
            reputation.update_utilization(utilization);
        }
    }

    /// Record request for a tenant
    pub fn record_request(&mut self, tenant_id: &CustomerId) {
        self.get_or_create(tenant_id.clone()).record_request();
    }

    /// Record fulfilled allocation
    pub fn record_fulfilled(&mut self, tenant_id: &CustomerId, allocated: f64, used: f64) {
        self.get_or_create(tenant_id.clone())
            .record_fulfilled(allocated, used);
    }

    /// Select tenant with highest priority (considering reputation)
    pub fn select_best_tenant(&self, candidates: &[CustomerId]) -> Option<CustomerId> {
        candidates
            .iter()
            .max_by(|a, b| {
                let priority_a = self.get(a).map(|r| r.scheduling_priority()).unwrap_or(0.0);
                let priority_b = self.get(b).map(|r| r.scheduling_priority()).unwrap_or(0.0);
                priority_a.partial_cmp(&priority_b).unwrap()
            })
            .cloned()
    }

    /// Get reputation statistics
    pub fn stats(&self) -> ReputationStats {
        let num_tenants = self.tenants.len();
        let reputations: Vec<f64> = self.tenants.values().map(|r| r.reputation).collect();

        let avg_reputation = if num_tenants > 0 {
            reputations.iter().sum::<f64>() / num_tenants as f64
        } else {
            0.0
        };

        let high_reputation = reputations.iter().filter(|&&r| r > 0.8).count();
        let low_reputation = reputations.iter().filter(|&&r| r < 0.5).count();

        ReputationStats {
            num_tenants,
            avg_reputation,
            high_reputation_count: high_reputation,
            low_reputation_count: low_reputation,
        }
    }
}

/// Reputation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationStats {
    pub num_tenants: usize,
    pub avg_reputation: f64,
    pub high_reputation_count: usize,
    pub low_reputation_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ground_core::CustomerId;

    #[test]
    fn test_reputation_update() {
        let mut reputation = TenantReputation::new("tenant1".into());

        assert_eq!(reputation.reputation, 1.0);

        // High utilization should maintain reputation
        reputation.record_fulfilled(100.0, 95.0);
        assert!(reputation.reputation > 0.9);

        // Low utilization should decrease reputation
        reputation.record_fulfilled(100.0, 30.0);
        reputation.record_fulfilled(100.0, 30.0);
        reputation.record_fulfilled(100.0, 30.0);
        reputation.record_fulfilled(100.0, 30.0);
        reputation.record_fulfilled(100.0, 30.0);
        assert!(reputation.reputation < 0.85);
    }

    #[test]
    fn test_reputation_tracker() {
        let mut tracker = ReputationTracker::new(0.1);

        let tenant1 = CustomerId::from("tenant1");
        let tenant2 = CustomerId::from("tenant2");

        tracker.record_fulfilled(&tenant1, 100.0, 90.0); // Good utilization
        tracker.record_fulfilled(&tenant2, 100.0, 30.0); // Poor utilization

        // Tenant1 should have higher priority
        let selected = tracker.select_best_tenant(&[tenant1.clone(), tenant2.clone()]);
        assert_eq!(selected, Some(tenant1));
    }
}

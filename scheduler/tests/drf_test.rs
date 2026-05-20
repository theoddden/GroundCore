#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use scheduler::{DominantResourceFairness, ResourceShare, ResourceType};

    #[test]
    fn test_resource_share_creation() {
        let share = ResourceShare::new(ResourceType::LBandSdrTime, 100.0);
        assert_eq!(share.resource_type, ResourceType::LBandSdrTime);
        assert_eq!(share.total_capacity, 100.0);
        assert_eq!(share.allocated, 0.0);
        assert_eq!(share.used, 0.0);
    }

    #[test]
    fn test_resource_share_utilization() {
        let mut share = ResourceShare::new(ResourceType::LBandSdrTime, 100.0);
        share.allocated = 50.0;
        share.used = 25.0;

        assert_eq!(share.share(), 0.5);
        assert_eq!(share.utilization(), 0.5);
    }

    #[test]
    fn test_drf_fairness_calculation() {
        let mut drf = DominantResourceFairness::new();
        drf.set_capacity(ResourceType::LBandSdrTime, 100.0);
        drf.set_capacity(ResourceType::RotatorHours, 50.0);

        let tenant1 = "tenant1".to_string();
        let tenant2 = "tenant2".to_string();

        let alloc1 = drf.get_or_create_allocation(tenant1.clone());
        alloc1.add_share(ResourceShare::new(ResourceType::LBandSdrTime, 100.0));
        alloc1.add_share(ResourceShare::new(ResourceType::RotatorHours, 50.0));
        alloc1.allocate(ResourceType::LBandSdrTime, 40.0);
        alloc1.allocate(ResourceType::RotatorHours, 30.0);

        let alloc2 = drf.get_or_create_allocation(tenant2.clone());
        alloc2.add_share(ResourceShare::new(ResourceType::LBandSdrTime, 100.0));
        alloc2.add_share(ResourceShare::new(ResourceType::RotatorHours, 50.0));
        alloc2.allocate(ResourceType::LBandSdrTime, 20.0);
        alloc2.allocate(ResourceType::RotatorHours, 10.0);

        // Tenant2 should have higher priority (lower dominant share)
        let priority1 = drf.scheduling_priority(&tenant1);
        let priority2 = drf.scheduling_priority(&tenant2);
        assert!(priority2 < priority1);
    }

    #[test]
    fn test_drf_allocation() {
        let mut drf = DominantResourceFairness::new();
        drf.set_capacity(ResourceType::LBandSdrTime, 100.0);

        let tenant1 = "tenant1".to_string();
        let alloc1 = drf.get_or_create_allocation(tenant1.clone());
        alloc1.add_share(ResourceShare::new(ResourceType::LBandSdrTime, 100.0));
        alloc1.allocate(ResourceType::LBandSdrTime, 30.0);

        let allocation = drf.get_allocation(&tenant1);
        assert!(allocation.is_some());
        assert_eq!(allocation.unwrap().dominant_share(), 0.3);
    }

    #[test]
    fn test_resource_type_display() {
        assert_eq!(format!("{:?}", ResourceType::LBandSdrTime), "LBandSdrTime");
        assert_eq!(format!("{:?}", ResourceType::SBandSdrTime), "SBandSdrTime");
        assert_eq!(format!("{:?}", ResourceType::RotatorHours), "RotatorHours");
        assert_eq!(format!("{:?}", ResourceType::EgressBandwidth), "EgressBandwidth");
        assert_eq!(format!("{:?}", ResourceType::TotalPasses), "TotalPasses");
    }
}

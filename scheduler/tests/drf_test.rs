#[cfg(test)]
mod tests {
    use ground_station_scheduler::drf::{DominantResourceFairness, ResourceType, ResourceShare};
    use std::collections::HashMap;

    #[test]
    fn test_resource_share_creation() {
        let share = ResourceShare {
            cpu: 0.5,
            memory: 0.3,
            bandwidth: 0.7,
            storage: 0.2,
        };
        
        assert_eq!(share.cpu, 0.5);
        assert_eq!(share.memory, 0.3);
    }

    #[test]
    fn test_drf_fairness_calculation() {
        let mut drf = DominantResourceFairness::new();
        
        let mut tenant1_resources = HashMap::new();
        tenant1_resources.insert(ResourceType::Cpu, 0.5);
        tenant1_resources.insert(ResourceType::Memory, 0.3);
        
        let mut tenant2_resources = HashMap::new();
        tenant2_resources.insert(ResourceType::Cpu, 0.3);
        tenant2_resources.insert(ResourceType::Memory, 0.5);
        
        let share1 = drf.calculate_dominant_share(&tenant1_resources);
        let share2 = drf.calculate_dominant_share(&tenant2_resources);
        
        // Both tenants should have equal dominant share (0.5)
        assert_eq!(share1, 0.5);
        assert_eq!(share2, 0.5);
    }

    #[test]
    fn test_drf_allocation() {
        let mut drf = DominantResourceFairness::new();
        
        let mut total_capacity = HashMap::new();
        total_capacity.insert(ResourceType::Cpu, 100.0);
        total_capacity.insert(ResourceType::Memory, 100.0);
        
        let mut allocation1 = HashMap::new();
        allocation1.insert(ResourceType::Cpu, 30.0);
        allocation1.insert(ResourceType::Memory, 20.0);
        
        let mut allocation2 = HashMap::new();
        allocation2.insert(ResourceType::Cpu, 20.0);
        allocation2.insert(ResourceType::Memory, 30.0);
        
        drf.set_total_capacity(total_capacity);
        drf.add_allocation("tenant1".to_string(), allocation1);
        drf.add_allocation("tenant2".to_string(), allocation2);
        
        let fair_share = drf.get_fair_share("tenant1");
        assert!(fair_share.is_some());
    }

    #[test]
    fn test_resource_type_display() {
        assert_eq!(format!("{:?}", ResourceType::Cpu), "Cpu");
        assert_eq!(format!("{:?}", ResourceType::Memory), "Memory");
        assert_eq!(format!("{:?}", ResourceType::Bandwidth), "Bandwidth");
        assert_eq!(format!("{:?}", ResourceType::Storage), "Storage");
    }
}

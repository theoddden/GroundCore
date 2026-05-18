#[cfg(test)]
mod tests {
    use ground_station_oisl::topology::control_node_placement::{
        ControlNodePlacementAlgorithm, PlacementError,
    };
    use ground_station_oisl::topology::{TopologyForecast, GraphSnapshot, NodeState, NodeType, Position3D, Velocity3D};
    use ground_station_oisl::{NodeId, SatelliteId};
    use chrono::{DateTime, Utc};
    use std::collections::{HashSet, HashMap};

    #[test]
    fn test_control_node_placement_algorithm_creation() {
        let algorithm = ControlNodePlacementAlgorithm::new(5, 10);
        assert_eq!(algorithm.num_clusters, 5);
        assert_eq!(algorithm.local_search_iterations, 10);
    }

    #[test]
    fn test_control_node_placement_default() {
        let algorithm = ControlNodePlacementAlgorithm::default();
        assert_eq!(algorithm.num_clusters, 5);
        assert_eq!(algorithm.local_search_iterations, 10);
    }

    #[test]
    fn test_control_node_placement_no_candidates() {
        let algorithm = ControlNodePlacementAlgorithm::default();
        let forecast = TopologyForecast::new();
        let candidates = HashSet::new();
        
        let result = algorithm.select_control_nodes(&forecast, candidates, 3);
        assert!(matches!(result, Err(PlacementError::NoCandidateStations)));
    }

    #[test]
    fn test_control_node_placement_insufficient_candidates() {
        let algorithm = ControlNodePlacementAlgorithm::default();
        let forecast = TopologyForecast::new();
        let mut candidates = HashSet::new();
        candidates.insert("station1".to_string());
        
        let result = algorithm.select_control_nodes(&forecast, candidates, 3);
        assert!(matches!(result, Err(PlacementError::InsufficientCandidates { .. })));
    }

    #[test]
    fn test_control_node_placement_no_snapshots() {
        let algorithm = ControlNodePlacementAlgorithm::default();
        let forecast = TopologyForecast::new();
        let mut candidates = HashSet::new();
        candidates.insert("station1".to_string());
        candidates.insert("station2".to_string());
        candidates.insert("station3".to_string());
        
        let result = algorithm.select_control_nodes(&forecast, candidates, 2);
        assert!(matches!(result, Err(PlacementError::NoSnapshots)));
    }

    #[test]
    fn test_control_node_placement_with_snapshots() {
        let mut algorithm = ControlNodePlacementAlgorithm::new(2, 5);
        let mut forecast = TopologyForecast::new();
        
        // Add some snapshots
        for i in 0..5 {
            let mut snapshot = GraphSnapshot::new(DateTime::from_timestamp(i * 3600, 0).unwrap());
            
            // Add satellite
            let sat_state = NodeState {
                node_type: NodeType::Satellite {
                    satellite_id: format!("SAT{}", i),
                },
                position: Position3D {
                    x_km: (i as f64) * 1000.0,
                    y_km: 0.0,
                    z_km: 7000.0,
                },
                velocity: Velocity3D {
                    x_km_s: 7.5,
                    y_km_s: 0.0,
                    z_km_s: 0.0,
                },
            };
            snapshot.add_node(format!("sat{}", i), sat_state);
            
            // Add ground stations
            for j in 0..3 {
                let station_state = NodeState {
                    node_type: NodeType::GroundStation {
                        station_id: format!("STATION{}", j),
                    },
                    position: Position3D {
                        x_km: (j as f64) * 100.0,
                        y_km: (j as f64) * 100.0,
                        z_km: 0.0,
                    },
                    velocity: Velocity3D {
                        x_km_s: 0.0,
                        y_km_s: 0.0,
                        z_km_s: 0.0,
                    },
                };
                snapshot.add_node(format!("station{}", j), station_state);
            }
            
            forecast.add_snapshot(snapshot);
        }
        
        let mut candidates = HashSet::new();
        candidates.insert("station0".to_string());
        candidates.insert("station1".to_string());
        candidates.insert("station2".to_string());
        
        let result = algorithm.select_control_nodes(&forecast, candidates, 2);
        assert!(result.is_ok());
        
        let selected = result.unwrap();
        assert_eq!(selected.len(), 2);
    }
}

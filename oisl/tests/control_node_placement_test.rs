#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use oisl::topology::control_node_placement::{
        ControlNodePlacementAlgorithm, PlacementError,
    };
    use oisl::topology::{
        GraphSnapshot, NodeState, NodeType, Position3D, TopologyForecast, Velocity3D,
    };
    use std::collections::{HashMap, HashSet};

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
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let end = DateTime::from_timestamp(3600, 0).unwrap();
        let forecast = TopologyForecast::new(start, end);
        let candidates = HashSet::new();

        let result = algorithm.select_control_nodes(&forecast, candidates, 3);
        assert!(matches!(result, Err(PlacementError::NoCandidateStations)));
    }

    #[test]
    fn test_control_node_placement_insufficient_candidates() {
        let algorithm = ControlNodePlacementAlgorithm::default();
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let end = DateTime::from_timestamp(3600, 0).unwrap();
        let forecast = TopologyForecast::new(start, end);
        let mut candidates = HashSet::new();
        candidates.insert("station1".to_string());

        let result = algorithm.select_control_nodes(&forecast, candidates, 3);
        assert!(matches!(
            result,
            Err(PlacementError::InsufficientCandidates { .. })
        ));
    }

    #[test]
    fn test_control_node_placement_no_snapshots() {
        let algorithm = ControlNodePlacementAlgorithm::default();
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let end = DateTime::from_timestamp(3600, 0).unwrap();
        let forecast = TopologyForecast::new(start, end);
        let mut candidates = HashSet::new();
        candidates.insert("station1".to_string());
        candidates.insert("station2".to_string());
        candidates.insert("station3".to_string());

        let result = algorithm.select_control_nodes(&forecast, candidates, 2);
        assert!(matches!(result, Err(PlacementError::NoSnapshots)));
    }

    #[test]
    fn test_control_node_placement_with_snapshots() {
        let algorithm = ControlNodePlacementAlgorithm::new(2, 5);
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let end = DateTime::from_timestamp(18000, 0).unwrap();
        let mut forecast = TopologyForecast::new(start, end);

        // Add some snapshots
        for i in 0..5 {
            let mut snapshot = GraphSnapshot {
                timestamp: DateTime::from_timestamp(i * 3600, 0).unwrap(),
                nodes: HashMap::new(),
                potential_edges: Vec::new(),
                active_links: Vec::new(),
            };

            // Add satellite
            let sat_state = NodeState {
                node_id: format!("sat{}", i),
                node_type: NodeType::Satellite {
                    satellite_id: format!("SAT{}", i),
                },
                position: Position3D {
                    x_km: (i as f64) * 1000.0,
                    y_km: 0.0,
                    z_km: 7000.0,
                },
                velocity: Velocity3D {
                    vx_kms: 7.5,
                    vy_kms: 0.0,
                    vz_kms: 0.0,
                },
                optical_terminals: Vec::new(),
            };
            snapshot.nodes.insert(format!("sat{}", i), sat_state);

            // Add ground stations
            for j in 0..3 {
                let station_state = NodeState {
                    node_id: format!("station{}", j),
                    node_type: NodeType::GroundStation {
                        station_id: format!("STATION{}", j),
                    },
                    position: Position3D {
                        x_km: (j as f64) * 100.0,
                        y_km: (j as f64) * 100.0,
                        z_km: 0.0,
                    },
                    velocity: Velocity3D {
                        vx_kms: 0.0,
                        vy_kms: 0.0,
                        vz_kms: 0.0,
                    },
                    optical_terminals: Vec::new(),
                };
                snapshot.nodes.insert(format!("station{}", j), station_state);
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

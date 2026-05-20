#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration};
    use oisl::topology::{
        GraphSnapshot, NodeState, NodeType, Position3D, TopologyForecast, Velocity3D,
    };
    use oisl::{NodeId, TimeWindow};

    #[test]
    fn test_graph_snapshot_creation() {
        let mut snapshot = GraphSnapshot::new(DateTime::from_timestamp(0, 0).unwrap());

        let node_id = "sat1".to_string();
        let node_state = NodeState {
            node_type: NodeType::Satellite {
                satellite_id: "SAT1".to_string(),
            },
            position: Position3D {
                x_km: 0.0,
                y_km: 0.0,
                z_km: 7000.0,
            },
            velocity: Velocity3D {
                x_km_s: 7.5,
                y_km_s: 0.0,
                z_km_s: 0.0,
            },
        };

        snapshot.add_node(node_id.clone(), node_state);
        assert!(snapshot.nodes.contains_key(&node_id));
    }

    #[test]
    fn test_topology_forecast_creation() {
        let forecast = TopologyForecast::new();
        assert_eq!(forecast.snapshots.len(), 0);
    }

    #[test]
    fn test_topology_forecast_add_snapshot() {
        let mut forecast = TopologyForecast::new();
        let snapshot = GraphSnapshot::new(DateTime::from_timestamp(0, 0).unwrap());
        forecast.add_snapshot(snapshot);
        assert_eq!(forecast.snapshots.len(), 1);
    }

    #[test]
    fn test_time_window_creation() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let end = DateTime::from_timestamp(3600, 0).unwrap();
        let window = TimeWindow::new(start, end);

        assert_eq!(window.duration(), Duration::seconds(3600));
        assert!(window.contains(DateTime::from_timestamp(1800, 0).unwrap()));
        assert!(!window.contains(DateTime::from_timestamp(7200, 0).unwrap()));
    }

    #[test]
    fn test_time_window_overlaps() {
        let window1 = TimeWindow::new(
            DateTime::from_timestamp(0, 0).unwrap(),
            DateTime::from_timestamp(3600, 0).unwrap(),
        );
        let window2 = TimeWindow::new(
            DateTime::from_timestamp(1800, 0).unwrap(),
            DateTime::from_timestamp(5400, 0).unwrap(),
        );

        assert!(window1.overlaps(&window2));
    }

    #[test]
    fn test_position_3d_distance() {
        let pos1 = Position3D {
            x_km: 0.0,
            y_km: 0.0,
            z_km: 0.0,
        };
        let pos2 = Position3D {
            x_km: 3.0,
            y_km: 4.0,
            z_km: 0.0,
        };

        let dx = pos2.x_km - pos1.x_km;
        let dy = pos2.y_km - pos1.y_km;
        let dz = pos2.z_km - pos1.z_km;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        assert_eq!(distance, 5.0);
    }
}

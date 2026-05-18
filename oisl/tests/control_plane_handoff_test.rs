#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use ground_station_oisl::federation::control_plane_handoff::{
        ControlPlaneHandoff, HandoffError, HandoffManager, HandoffMetrics, HandoffRequest,
        HandoffState, HandoffStep,
    };
    use ground_station_oisl::{NodeId, SatelliteId};
    use uuid::Uuid;

    #[test]
    fn test_handoff_request_creation() {
        let request = HandoffRequest {
            request_id: Uuid::new_v4(),
            satellite_id: SatelliteId::from("SAT1"),
            from_node: NodeId::from("node1"),
            to_node: NodeId::from("node2"),
            requested_at: Utc::now(),
            reason: "geometric degradation".to_string(),
        };

        assert_eq!(request.from_node, NodeId::from("node1"));
        assert_eq!(request.to_node, NodeId::from("node2"));
    }

    #[test]
    fn test_control_plane_handoff_creation() {
        let handoff = ControlPlaneHandoff::new(
            Uuid::new_v4(),
            SatelliteId::from("SAT1"),
            NodeId::from("node1"),
            NodeId::from("node2"),
        );

        assert_eq!(handoff.current_state(), HandoffState::RequestSubmitted);
    }

    #[test]
    fn test_handoff_state_transitions() {
        let mut handoff = ControlPlaneHandoff::new(
            Uuid::new_v4(),
            SatelliteId::from("SAT1"),
            NodeId::from("node1"),
            NodeId::from("node2"),
        );

        // Progress through states
        handoff
            .transition_to(HandoffState::RequestAcknowledged)
            .unwrap();
        assert_eq!(handoff.current_state(), HandoffState::RequestAcknowledged);

        handoff
            .transition_to(HandoffState::CertificateExchangeInitiated)
            .unwrap();
        assert_eq!(
            handoff.current_state(),
            HandoffState::CertificateExchangeInitiated
        );

        handoff
            .transition_to(HandoffState::CertificateExchangeCompleted)
            .unwrap();
        assert_eq!(
            handoff.current_state(),
            HandoffState::CertificateExchangeCompleted
        );

        handoff
            .transition_to(HandoffState::StateTransferInitiated)
            .unwrap();
        assert_eq!(
            handoff.current_state(),
            HandoffState::StateTransferInitiated
        );

        handoff
            .transition_to(HandoffState::StateTransferCompleted)
            .unwrap();
        assert_eq!(
            handoff.current_state(),
            HandoffState::StateTransferCompleted
        );

        handoff
            .transition_to(HandoffState::TrafficRedirectInitiated)
            .unwrap();
        assert_eq!(
            handoff.current_state(),
            HandoffState::TrafficRedirectInitiated
        );

        handoff
            .transition_to(HandoffState::TrafficRedirectCompleted)
            .unwrap();
        assert_eq!(
            handoff.current_state(),
            HandoffState::TrafficRedirectCompleted
        );

        handoff
            .transition_to(HandoffState::OldNodeReleased)
            .unwrap();
        assert_eq!(handoff.current_state(), HandoffState::OldNodeReleased);

        handoff
            .transition_to(HandoffState::HandoffComplete)
            .unwrap();
        assert_eq!(handoff.current_state(), HandoffState::HandoffComplete);
    }

    #[test]
    fn test_handoff_invalid_transition() {
        let mut handoff = ControlPlaneHandoff::new(
            Uuid::new_v4(),
            SatelliteId::from("SAT1"),
            NodeId::from("node1"),
            NodeId::from("node2"),
        );

        // Try to skip to a non-sequential state
        let result = handoff.transition_to(HandoffState::HandoffComplete);
        assert!(matches!(
            result,
            Err(HandoffError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn test_handoff_manager_creation() {
        let manager = HandoffManager::new();
        assert_eq!(manager.active_handoffs(), 0);
    }

    #[test]
    fn test_handoff_manager_initiate_handoff() {
        let mut manager = HandoffManager::new();

        let request = HandoffRequest {
            request_id: Uuid::new_v4(),
            satellite_id: SatelliteId::from("SAT1"),
            from_node: NodeId::from("node1"),
            to_node: NodeId::from("node2"),
            requested_at: Utc::now(),
            reason: "geometric degradation".to_string(),
        };

        let handoff_id = manager.initiate_handoff(request).unwrap();
        assert_eq!(manager.active_handoffs(), 1);

        let handoff = manager.get_handoff(handoff_id).unwrap();
        assert_eq!(handoff.current_state(), HandoffState::RequestSubmitted);
    }

    #[test]
    fn test_handoff_metrics() {
        let metrics = HandoffMetrics::default();
        assert_eq!(metrics.total_handoffs, 0);
        assert_eq!(metrics.successful_handoffs, 0);
        assert_eq!(metrics.failed_handoffs, 0);
        assert_eq!(metrics.average_duration_ms, 0.0);
    }

    #[test]
    fn test_handoff_serialization() {
        let handoff = ControlPlaneHandoff::new(
            Uuid::new_v4(),
            SatelliteId::from("SAT1"),
            NodeId::from("node1"),
            NodeId::from("node2"),
        );

        let serialized = serde_json::to_string(&handoff).unwrap();
        let deserialized: ControlPlaneHandoff = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.satellite_id(), handoff.satellite_id());
        assert_eq!(deserialized.from_node(), handoff.from_node());
        assert_eq!(deserialized.to_node(), handoff.to_node());
    }
}

#[cfg(test)]
mod tests {
    use ground_station_optical::link::state_machine::{
        DegradationAction, DegradationReason, FailureCause, LinkMetrics, LinkPhase, LinkQuality,
        OpticalLink, RecoveryStrategy, TerminationReason,
    };
    use ground_station_optical::oct::{LinkType, OctConfiguration};
    use uuid::Uuid;

    #[test]
    fn test_link_quality_excellent() {
        let quality = LinkQuality::excellent();
        assert!(quality.bit_error_rate < 1e-10);
        assert!(quality.signal_to_noise_db > 15.0);
        assert!(quality.pointing_error_urad < 20.0);
        assert!(quality.is_healthy());
    }

    #[test]
    fn test_link_quality_good() {
        let quality = LinkQuality::good();
        assert!(quality.bit_error_rate < 1e-8);
        assert!(quality.signal_to_noise_db > 10.0);
        assert!(quality.is_healthy());
    }

    #[test]
    fn test_link_quality_degraded() {
        let quality = LinkQuality::degraded();
        assert!(!quality.is_healthy());
        assert!(quality.bit_error_rate > 1e-7);
    }

    #[test]
    fn test_link_phase_transitions() {
        let config = OctConfiguration::default_s2s();
        let mut link = OpticalLink::new(
            Uuid::new_v4(),
            ("terminal1".to_string(), "terminal2".to_string()),
            config,
        );

        assert!(!link.is_established());

        link.transition_to_established(LinkQuality::good());
        assert!(link.is_established());
        assert!(link.is_healthy());

        link.transition_to_degrading(
            DegradationReason::AtmosphericTurbulence {
                severity: "moderate".to_string(),
            },
            DegradationAction::Monitor,
        );

        link.transition_to_failed(FailureCause::SignalLost);
        assert!(!link.is_established());
    }

    #[test]
    fn test_link_metrics_default() {
        let metrics = LinkMetrics::default();
        assert_eq!(metrics.data_rate_actual, 0);
        assert_eq!(metrics.data_rate_capacity, 10_000_000_000);
        assert_eq!(metrics.bit_error_rate, 0.0);
    }

    #[test]
    fn test_link_metrics_utilization() {
        let mut metrics = LinkMetrics::default();
        metrics.data_rate_actual = 5_000_000_000;
        assert_eq!(metrics.utilization(), 0.5);
    }

    #[test]
    fn test_link_metrics_control_plane() {
        let mut metrics = LinkMetrics::default();
        assert!(!metrics.control_plane_traffic.is_enabled());

        metrics.enable_control_plane(
            ground_station_optical::link::state_machine::TrafficPriority::Critical,
        );
        assert!(metrics.control_plane_traffic.is_enabled());

        metrics.update_control_plane_metrics(1_000_000, 5.0, 1000);
        assert_eq!(metrics.control_plane_traffic.data_rate_actual, 1_000_000);
        assert_eq!(metrics.control_plane_traffic.latency_ms, 5.0);
    }
}

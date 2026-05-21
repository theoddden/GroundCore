//! Integration test for digital twin divergence detection
//!
//! This test injects a known divergence and asserts that the loop catches it.
//! This is the verification scaffolding that proves the implementation is real
//! and not vibes.

use chrono::{Duration, Utc};
use digital_twin::{
    create_twin_channels, DivergenceDetector, Interval, MetricDivergence, MetricType, ObservedMetric,
};
use ground_core::LinkId;
use std::time::Duration as StdDuration;
use tokio::time::sleep;

#[tokio::test]
async fn test_divergence_detection_integration() {
    // Setup channels
    let (observed_tx, mut observed_rx, anomaly_tx, mut anomaly_rx, _forecast_tx, _forecast_rx) =
        create_twin_channels();

    // Spawn a task to process observations
    let processor_task = tokio::spawn(async move {
        let mut detector = DivergenceDetector::new(3.0);
        let link_id = LinkId::new();
        let base_time = Utc::now();

        // Process observations
        while let Some(observed) = observed_rx.recv().await {
            let predicted_interval = Interval::new(10.0, 20.0); // Expected SNR: 10-20 dB

            let result = detector.process_observation(
                observed.link_id.clone(),
                observed.metric_type,
                predicted_interval,
                observed.value,
                base_time,
                observed.observation_time,
            );

            if let Some(divergence) = result {
                // Send anomaly signal
                let signal = digital_twin::AnomalySignal {
                    divergence,
                    signal_time: Utc::now(),
                };
                if let Err(e) = anomaly_tx.send(signal).await {
                    panic!("Failed to send anomaly signal: {}", e);
                }
            }
        }
    });

    // Inject normal observation (within interval)
    let link_id = LinkId::new();
    observed_tx
        .send(ObservedMetric {
            link_id: link_id.clone(),
            metric_type: MetricType::Snr,
            value: 15.0, // Within [10, 20]
            observation_time: Utc::now(),
        })
        .await
        .unwrap();

    // Wait a bit
    sleep(StdDuration::from_millis(100)).await;

    // No anomaly should be detected yet
    let anomaly_result = tokio::time::timeout(StdDuration::from_millis(100), anomaly_rx.recv()).await;
    assert!(anomaly_result.is_err(), "No anomaly should be detected for normal observation");

    // Inject divergent observation (outside interval)
    observed_tx
        .send(ObservedMetric {
            link_id: link_id.clone(),
            metric_type: MetricType::Snr,
            value: 25.0, // Outside [10, 20] - this is an anomaly!
            observation_time: Utc::now(),
        })
        .await
        .unwrap();

    // Anomaly should be detected
    let anomaly_result = tokio::time::timeout(StdDuration::from_millis(500), anomaly_rx.recv()).await;
    assert!(anomaly_result.is_ok(), "Anomaly should be detected for divergent observation");

    let anomaly_signal = anomaly_result.unwrap().unwrap();
    assert_eq!(anomaly_signal.divergence.link_id, link_id);
    assert_eq!(anomaly_signal.divergence.metric_type, MetricType::Snr);
    assert_eq!(anomaly_signal.divergence.observed, 25.0);
    assert!(anomaly_signal.divergence.is_anomaly());

    // Cleanup
    drop(observed_tx);
    processor_task.await.unwrap();
}

#[tokio::test]
async fn test_cusum_change_point_detection() {
    let mut detector = DivergenceDetector::new(3.0);
    let link_id = LinkId::new();
    let base_time = Utc::now();
    let predicted_interval = Interval::new(10.0, 20.0);

    // Send normal observations
    for i in 0..10 {
        let result = detector.process_observation(
            link_id.clone(),
            MetricType::Snr,
            predicted_interval,
            15.0, // Normal
            base_time,
            base_time + Duration::seconds(i),
        );
        assert!(result.is_none(), "No anomaly for normal observations");
    }

    // Send divergent observations to trigger CUSUM
    let mut change_point_detected = false;
    for i in 10..20 {
        let result = detector.process_observation(
            link_id.clone(),
            MetricType::Snr,
            predicted_interval,
            25.0, // Divergent
            base_time,
            base_time + Duration::seconds(i),
        );

        if let Some(divergence) = result {
            if divergence.change_point_detected_at.is_some() {
                change_point_detected = true;
                println!(
                    "Change point detected at: {:?}",
                    divergence.change_point_detected_at
                );
                break;
            }
        }
    }

    assert!(
        change_point_detected,
        "CUSUM should detect the change point after sustained divergence"
    );
}

#[tokio::test]
async fn test_bounded_interval_set_membership() {
    // Test that bounded intervals provide provable anomaly detection
    let interval = Interval::new(10.0, 20.0);

    // Values within interval are NOT anomalies
    assert!(interval.contains(15.0));
    assert_eq!(interval.distance(15.0), 0.0);

    // Values outside interval ARE anomalies (provable by set-membership)
    assert!(!interval.contains(5.0));
    assert_eq!(interval.distance(5.0), 5.0); // 5 units below lower bound

    assert!(!interval.contains(25.0));
    assert_eq!(interval.distance(25.0), 5.0); // 5 units above upper bound

    // This is the defensible version of a sigma gate - no probabilistic guess
    let detector = DivergenceDetector::new(3.0);
    let link_id = LinkId::new();
    let base_time = Utc::now();

    // Observation within interval - no anomaly
    let result = detector.process_observation(
        link_id.clone(),
        MetricType::Snr,
        interval,
        15.0,
        base_time,
        base_time,
    );
    assert!(result.is_none());

    // Observation outside interval - provable anomaly
    let result = detector.process_observation(
        link_id.clone(),
        MetricType::Snr,
        interval,
        25.0,
        base_time,
        base_time,
    );
    assert!(result.is_some());
    assert!(result.unwrap().is_anomaly());
}

#[tokio::test]
async fn test_bi_temporal_divergence_logging() {
    let detector = DivergenceDetector::new(3.0);
    let link_id = LinkId::new();
    let prediction_time = Utc::now();
    let observation_time = prediction_time + Duration::seconds(1);
    let interval = Interval::new(10.0, 20.0);

    // Process a divergent observation
    let result = detector.process_observation(
        link_id.clone(),
        MetricType::Snr,
        interval,
        25.0,
        prediction_time,
        observation_time,
    );

    assert!(result.is_some());
    let divergence = result.unwrap();

    // Verify bi-temporal timestamps are recorded
    assert_eq!(
        *divergence.bitemporal_stamp.event_time(),
        prediction_time
    );
    assert_eq!(
        *divergence.bitemporal_stamp.observation_time(),
        observation_time
    );

    // This enables replay: "what did the system predict at T, and when did we observe reality?"
    // Critical for incident review, insurance disputes, and regulatory audits
}

#[tokio::test]
async fn test_snapshot_manager_forensic_capability() {
    use digital_twin::SnapshotManager;
    use bevy_ecs::World;

    let mut manager = SnapshotManager::new(10);
    let base_time = Utc::now();

    // Store snapshots over time
    for i in 0..5 {
        let snapshot = digital_twin::TwinSnapshot::new(
            base_time + Duration::seconds(i),
            World::new(),
            Vec::new(),
        );
        manager.store_snapshot(snapshot);
    }

    // Query: "what did the system believe at T?"
    let query_time = base_time + Duration::seconds(2);
    let snapshot = manager.get_closest_snapshot(query_time);
    assert!(snapshot.is_some());
    assert_eq!(snapshot.unwrap().timestamp, query_time);

    // This is the forensic feature Aalyria structurally can't match
    // Because every tick is bi-temporally stamped, you can replay what-we-knew-when
}

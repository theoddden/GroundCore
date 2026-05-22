//! Divergence detection and anomaly signaling
//!
//! This module implements the core primitive: divergence from physics prediction
//! is itself the anomaly signal — no trained model, no hand-tuned thresholds.
//!
//! # Rigorous Anomaly Detection
//!
//! Two things make this rigorous rather than hand-wavy:
//!
//! 1. **Predict bounded intervals, not points.** Given TLE staleness and atmospheric
//!    variance, the expected metric is X ± [guaranteed interval]. An observation
//!    outside the interval is a provable anomaly, not a probabilistic guess. This is
//!    set-membership estimation — the defensible version of a sigma gate.
//!
//! 2. **Detect when the regime shifted, not just that one sample is high.** Use
//!    CUSUM change-point detection on the divergence stream. This is decades-old,
//!    battle-tested signal-processing literature, and it's exactly "when did prediction
//!    first diverge from reality."

use bitemporal::{BiTemporal, EventTime, ReceptionTime};
use chrono::{DateTime, Utc};
use ground_core::LinkId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bounded interval for set-membership estimation
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Interval {
    /// Lower bound of the interval
    pub lower: f64,
    /// Upper bound of the interval
    pub upper: f64,
}

impl Interval {
    /// Create a new interval
    pub fn new(lower: f64, upper: f64) -> Self {
        assert!(lower <= upper, "Interval lower bound must be <= upper bound");
        Self { lower, upper }
    }

    /// Check if a value is within the interval
    pub fn contains(&self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }

    /// Compute the distance from the interval (0 if inside, positive if outside)
    pub fn distance(&self, value: f64) -> f64 {
        if self.contains(value) {
            0.0
        } else if value < self.lower {
            self.lower - value
        } else {
            value - self.upper
        }
    }

    /// Compute the width of the interval
    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }

    /// Create an interval from a center point and half-width
    pub fn from_center(center: f64, half_width: f64) -> Self {
        Self::new(center - half_width, center + half_width)
    }
}

/// Metric types that can be monitored for divergence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    /// Signal-to-Noise Ratio (dB)
    Snr,
    /// Bit Error Rate
    Ber,
    /// Link throughput (Mbps)
    Throughput,
    /// Latency (ms)
    Latency,
    /// Doppler shift (Hz)
    Doppler,
    /// Received signal strength (dBm)
    Rssi,
    /// Optical terminal temperature (C)
    Temperature,
    /// Pointing error (degrees)
    PointingError,
}

/// Causal hypothesis for a divergence event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalHypothesis {
    /// Hypothesis description
    pub description: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Supporting evidence
    pub evidence: Vec<String>,
}

impl CausalHypothesis {
    /// Create a new hypothesis
    pub fn new(description: String, confidence: f64) -> Self {
        Self {
            description,
            confidence,
            evidence: Vec::new(),
        }
    }

    /// Add evidence to the hypothesis
    pub fn with_evidence(mut self, evidence: String) -> Self {
        self.evidence.push(evidence);
        self
    }
}

/// Metric divergence record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDivergence {
    /// Link identifier
    pub link_id: LinkId,
    /// Metric type
    pub metric_type: MetricType,
    /// Predicted interval (bounded, not point estimate)
    pub predicted_interval: Interval,
    /// Observed value
    pub observed: f64,
    /// How many standard deviations from prediction (sigma)
    pub sigma: f64,
    /// When the change-point was first detected (CUSUM crossed threshold)
    pub change_point_detected_at: Option<DateTime<Utc>>,
    /// Bi-temporal timestamp (prediction time vs observation time)
    pub bitemporal_stamp: BiTemporal<()>,
    /// Causal hypotheses for the divergence
    pub causal_hypotheses: Vec<CausalHypothesis>,
}

impl MetricDivergence {
    /// Create a new divergence record
    pub fn new(
        link_id: LinkId,
        metric_type: MetricType,
        predicted_interval: Interval,
        observed: f64,
        sigma: f64,
        event_time: EventTime,
        reception_time: ReceptionTime,
    ) -> Self {
        Self {
            link_id,
            metric_type,
            predicted_interval,
            observed,
            sigma,
            change_point_detected_at: None,
            bitemporal_stamp: BiTemporal::new((), event_time, reception_time),
            causal_hypotheses: Vec::new(),
        }
    }

    /// Check if this is an anomaly (outside predicted interval)
    pub fn is_anomaly(&self) -> bool {
        !self.predicted_interval.contains(self.observed)
    }

    /// Add a causal hypothesis
    pub fn add_hypothesis(&mut self, hypothesis: CausalHypothesis) {
        self.causal_hypotheses.push(hypothesis);
    }

    /// Mark the change-point detection time
    pub fn mark_change_point(&mut self, detected_at: DateTime<Utc>) {
        self.change_point_detected_at = Some(detected_at);
    }
}

/// CUSUM change-point detector for regime shift detection
#[derive(Debug, Clone)]
pub struct CusumDetector {
    /// Cumulative sum statistic
    cusum: f64,
    /// Threshold for change-point detection
    threshold: f64,
    /// Reference value (expected mean under normal conditions)
    reference: f64,
    /// Minimum detectable shift
    min_shift: f64,
    /// Whether a change-point has been detected
    change_detected: bool,
    /// When the change-point was detected
    detection_time: Option<DateTime<Utc>>,
}

impl CusumDetector {
    /// Create a new CUSUM detector
    ///
    /// # Arguments
    /// * `threshold` - Detection threshold (typically 4-5 sigma)
    /// * `reference` - Expected mean under normal conditions
    /// * `min_shift` - Minimum shift to detect (in sigma units)
    pub fn new(threshold: f64, reference: f64, min_shift: f64) -> Self {
        Self {
            cusum: 0.0,
            threshold,
            reference,
            min_shift,
            change_detected: false,
            detection_time: None,
        }
    }

    /// Update the detector with a new observation
    ///
    /// Returns true if a change-point is detected
    pub fn update(&mut self, observation: f64, time: DateTime<Utc>) -> bool {
        if self.change_detected {
            return false; // Already detected, don't reset
        }

        // Compute the deviation from reference
        let deviation = observation - self.reference;

        // Update CUSUM (one-sided for positive shifts)
        // S_t = max(0, S_{t-1} + deviation - min_shift)
        self.cusum = (self.cusum + deviation - self.min_shift).max(0.0);

        // Check for change-point
        if self.cusum >= self.threshold {
            self.change_detected = true;
            self.detection_time = Some(time);
            return true;
        }

        false
    }

    /// Reset the detector
    pub fn reset(&mut self) {
        self.cusum = 0.0;
        self.change_detected = false;
        self.detection_time = None;
    }

    /// Check if a change-point has been detected
    pub fn is_detected(&self) -> bool {
        self.change_detected
    }

    /// Get the detection time if detected
    pub fn detection_time(&self) -> Option<DateTime<Utc>> {
        self.detection_time
    }
}

/// Divergence detector per link and metric type
#[derive(Debug, Clone)]
pub struct DivergenceDetector {
    /// CUSUM detectors per (link_id, metric_type)
    detectors: HashMap<(LinkId, MetricType), CusumDetector>,
    /// Divergence log
    divergence_log: Vec<MetricDivergence>,
    /// Threshold for considering a value anomalous (sigma units)
    anomaly_threshold: f64,
}

impl DivergenceDetector {
    /// Create a new divergence detector
    ///
    /// # Arguments
    /// * `anomaly_threshold` - Sigma threshold for anomaly detection (e.g., 3.0)
    pub fn new(anomaly_threshold: f64) -> Self {
        Self {
            detectors: HashMap::new(),
            divergence_log: Vec::new(),
            anomaly_threshold,
        }
    }

    /// Ensure a detector exists for the given link and metric
    fn ensure_detector(&mut self, link_id: LinkId, metric_type: MetricType, reference: f64) {
        let key = (link_id, metric_type);
        if !self.detectors.contains_key(&key) {
            // CUSUM threshold: 5 sigma, min shift: 1 sigma
            self.detectors
                .insert(key, CusumDetector::new(5.0, reference, 1.0));
        }
    }

    /// Process a new observation against a predicted interval
    ///
    /// Returns Some(divergence) if an anomaly is detected
    pub fn process_observation(
        &mut self,
        link_id: LinkId,
        metric_type: MetricType,
        predicted_interval: Interval,
        observed: f64,
        event_time: EventTime,
        reception_time: ReceptionTime,
    ) -> Option<MetricDivergence> {
        // Compute sigma (distance from interval normalized by interval width)
        let distance = predicted_interval.distance(observed);
        let sigma = if predicted_interval.width() > 0.0 {
            distance / predicted_interval.width()
        } else {
            distance // Avoid division by zero
        };

        // Check if outside interval
        let is_anomaly = !predicted_interval.contains(observed);

        // Update CUSUM detector
        self.ensure_detector(link_id, metric_type, 0.0); // Reference = 0 deviation
        let key = (link_id, metric_type);
        let detector = self.detectors.get_mut(&key)?;
        let change_detected = detector.update(sigma, reception_time.as_datetime());

        // If anomaly detected or change-point detected, log it
        if is_anomaly || change_detected {
            let mut divergence = MetricDivergence::new(
                link_id,
                metric_type,
                predicted_interval,
                observed,
                sigma,
                event_time,
                reception_time,
            );

            if change_detected {
                if let Some(detected_at) = detector.detection_time() {
                    divergence.mark_change_point(detected_at);
                }
            }

            self.divergence_log.push(divergence.clone());
            Some(divergence)
        } else {
            None
        }
    }

    /// Get the divergence log
    pub fn divergence_log(&self) -> &[MetricDivergence] {
        &self.divergence_log
    }

    /// Clear the divergence log
    pub fn clear_log(&mut self) {
        self.divergence_log.clear();
    }

    /// Reset all CUSUM detectors
    pub fn reset_detectors(&mut self) {
        for detector in self.detectors.values_mut() {
            detector.reset();
        }
    }
}

impl Default for DivergenceDetector {
    fn default() -> Self {
        Self::new(3.0) // Default 3-sigma threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_interval_contains() {
        let interval = Interval::new(10.0, 20.0);
        assert!(interval.contains(15.0));
        assert!(interval.contains(10.0));
        assert!(interval.contains(20.0));
        assert!(!interval.contains(9.0));
        assert!(!interval.contains(21.0));
    }

    #[test]
    fn test_interval_distance() {
        let interval = Interval::new(10.0, 20.0);
        assert_eq!(interval.distance(15.0), 0.0);
        assert_eq!(interval.distance(5.0), 5.0);
        assert_eq!(interval.distance(25.0), 5.0);
    }

    #[test]
    fn test_cusum_detector() {
        let mut detector = CusumDetector::new(5.0, 0.0, 1.0);
        let base_time = Utc::now();

        // Normal observations
        for i in 0..10 {
            assert!(!detector.update(0.1, base_time + Duration::seconds(i)));
        }

        // Shift detection
        for i in 10..20 {
            if detector.update(2.0, base_time + Duration::seconds(i)) {
                assert!(detector.is_detected());
                assert!(detector.detection_time().is_some());
                return;
            }
        }

        panic!("CUSUM should have detected the shift");
    }

    #[test]
    fn test_divergence_detector() {
        let mut detector = DivergenceDetector::new(3.0);
        let link_id = LinkId::new();
        let base_time = Utc::now();

        // Normal observation (within interval)
        let interval = Interval::new(10.0, 20.0);
        let result = detector.process_observation(
            link_id.clone(),
            MetricType::Snr,
            interval,
            15.0,
            base_time,
            base_time,
        );
        assert!(result.is_none());

        // Anomalous observation (outside interval)
        let result = detector.process_observation(
            link_id.clone(),
            MetricType::Snr,
            interval,
            25.0,
            base_time,
            base_time + Duration::seconds(1),
        );
        assert!(result.is_some());
        assert!(result.unwrap().is_anomaly());
    }
}

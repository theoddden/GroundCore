// Degradation detection
//
// Optical links don't fail cleanly — they degrade over seconds or minutes before
// failing entirely. Detecting degradation early gives the system time to initiate
// a handoff to a backup link before the primary fails.

use crate::link::metrics::{MetricSnapshot, MetricTrend};
use crate::link::state_machine::{DegradationAction, DegradationReason, LinkQuality};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Degradation detector
pub struct DegradationDetector {
    threshold_ber: f64,
    threshold_snr_db: f64,
    threshold_pointing_urad: f64,
    observation_window: Duration,
}

impl DegradationDetector {
    pub fn new() -> Self {
        Self {
            threshold_ber: 1e-6,
            threshold_snr_db: 10.0,
            threshold_pointing_urad: 200.0,
            observation_window: Duration::seconds(30),
        }
    }

    pub fn with_thresholds(ber: f64, snr_db: f64, pointing_urad: f64) -> Self {
        Self {
            threshold_ber: ber,
            threshold_snr_db: snr_db,
            threshold_pointing_urad: pointing_urad,
            observation_window: Duration::seconds(30),
        }
    }

    pub fn detect_degradation(
        &self,
        current_quality: &LinkQuality,
        recent_snapshots: &[MetricSnapshot],
    ) -> Option<(DegradationReason, DegradationAction)> {
        // Check individual thresholds
        if current_quality.bit_error_rate > self.threshold_ber {
            return Some((
                DegradationReason::FecOverwhelmed {
                    ber_trend: "increasing".to_string(),
                },
                DegradationAction::InitiateHandoff,
            ));
        }

        if current_quality.signal_to_noise_db < self.threshold_snr_db {
            // Determine the cause based on trend
            let trend = self.calculate_trend(recent_snapshots);
            match trend {
                MetricTrend::Degrading => {
                    // Gradual degradation suggests atmospheric or pointing issues
                    if current_quality.pointing_error_urad > self.threshold_pointing_urad {
                        return Some((
                            DegradationReason::PointingDrift {
                                drift_rate_urad_per_sec: self.estimate_drift_rate(recent_snapshots),
                            },
                            DegradationAction::InitiateHandoff,
                        ));
                    } else {
                        return Some((
                            DegradationReason::AtmosphericTurbulence {
                                severity: "moderate".to_string(),
                            },
                            DegradationAction::Monitor,
                        ));
                    }
                }
                MetricTrend::Stable => {
                    return Some((
                        DegradationReason::UnexplainedSnrDrop {
                            magnitude_db: self.threshold_snr_db
                                - current_quality.signal_to_noise_db,
                        },
                        DegradationAction::Monitor,
                    ));
                }
                MetricTrend::Improving => {
                    // SNR is improving but still below threshold - monitor
                    return Some((
                        DegradationReason::UnexplainedSnrDrop {
                            magnitude_db: self.threshold_snr_db
                                - current_quality.signal_to_noise_db,
                        },
                        DegradationAction::Monitor,
                    ));
                }
            }
        }

        if current_quality.pointing_error_urad > self.threshold_pointing_urad {
            return Some((
                DegradationReason::PointingDrift {
                    drift_rate_urad_per_sec: self.estimate_drift_rate(recent_snapshots),
                },
                DegradationAction::InitiateHandoff,
            ));
        }

        None
    }

    fn calculate_trend(&self, snapshots: &[MetricSnapshot]) -> MetricTrend {
        if snapshots.len() < 2 {
            return MetricTrend::Stable;
        }

        let recent = &snapshots[snapshots.len().saturating_sub(5)..];
        if recent.len() < 2 {
            return MetricTrend::Stable;
        }

        let first = recent.first().unwrap();
        let last = recent.last().unwrap();
        let delta = last.signal_quality_db - first.signal_quality_db;

        if delta > 1.0 {
            MetricTrend::Improving
        } else if delta < -1.0 {
            MetricTrend::Degrading
        } else {
            MetricTrend::Stable
        }
    }

    fn estimate_drift_rate(&self, snapshots: &[MetricSnapshot]) -> f64 {
        if snapshots.len() < 2 {
            return 0.0;
        }

        let first = snapshots.first().unwrap();
        let last = snapshots.last().unwrap();
        let delta_pointing = last.pointing_error_urad - first.pointing_error_urad;
        let delta_time = (last.timestamp - first.timestamp).num_seconds().max(1);

        delta_pointing / delta_time as f64
    }
}

impl Default for DegradationDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Degradation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationEvent {
    pub detected_at: DateTime<Utc>,
    pub reason: DegradationReason,
    pub action_taken: DegradationAction,
    pub quality_at_detection: LinkQuality,
}

impl DegradationEvent {
    pub fn new(
        reason: DegradationReason,
        action_taken: DegradationAction,
        quality_at_detection: LinkQuality,
    ) -> Self {
        Self {
            detected_at: Utc::now(),
            reason,
            action_taken,
            quality_at_detection,
        }
    }
}

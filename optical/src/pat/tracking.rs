// Closed-loop tracking
//
// Once acquisition succeeds, the system enters tracking mode where it maintains
// lock by continuously adjusting pointing based on beacon feedback.

use serde::{Deserialize, Serialize};

/// Tracking quality metrics
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackingQuality {
    pub pointing_error_urad: f64,
    pub signal_to_noise_db: f64,
    pub lock_confidence: f64,
}

impl TrackingQuality {
    pub fn new(pointing_error_urad: f64, signal_to_noise_db: f64, lock_confidence: f64) -> Self {
        Self {
            pointing_error_urad,
            signal_to_noise_db,
            lock_confidence,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.pointing_error_urad < 100.0
            && self.signal_to_noise_db > 10.0
            && self.lock_confidence > 0.9
    }

    pub fn is_degrading(&self) -> bool {
        self.pointing_error_urad > 50.0 || self.signal_to_noise_db < 12.0
    }

    pub fn is_critical(&self) -> bool {
        self.pointing_error_urad > 200.0 || self.signal_to_noise_db < 5.0
    }
}

/// Tracking metrics (more detailed than quality)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingMetrics {
    pub pointing_error_urad: f64,
    pub signal_to_noise_db: f64,
    pub lock_confidence: f64,
    pub data_rate_actual: u64,
    pub bit_error_rate: f64,
    pub fec_corrections_per_second: u64,
    pub arq_retransmits_per_second: u64,
    pub beacon_power_dbm: f64,
    pub doppler_shift_hz: f64,
}

impl TrackingMetrics {
    pub fn from_quality(quality: TrackingQuality) -> Self {
        Self {
            pointing_error_urad: quality.pointing_error_urad,
            signal_to_noise_db: quality.signal_to_noise_db,
            lock_confidence: quality.lock_confidence,
            data_rate_actual: 0,
            bit_error_rate: 0.0,
            fec_corrections_per_second: 0,
            arq_retransmits_per_second: 0,
            beacon_power_dbm: -30.0,
            doppler_shift_hz: 0.0,
        }
    }

    pub fn healthy(&self) -> bool {
        self.pointing_error_urad < 100.0
            && self.signal_to_noise_db > 10.0
            && self.lock_confidence > 0.9
    }

    pub fn degrade_score(&self) -> f64 {
        let pointing_score = (self.pointing_error_urad / 200.0).min(1.0);
        let snr_score = 1.0 - (self.signal_to_noise_db / 20.0).max(0.0);
        let lock_score = 1.0 - self.lock_confidence;
        (pointing_score + snr_score + lock_score) / 3.0
    }
}

/// Tracking handle for active tracking
#[derive(Debug, Clone)]
pub struct TrackingHandle {
    pub tracking_id: uuid::Uuid,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub current_metrics: TrackingMetrics,
    pub is_active: bool,
}

impl Default for TrackingHandle {
    fn default() -> Self {
        Self {
            tracking_id: uuid::Uuid::new_v4(),
            started_at: chrono::Utc::now(),
            current_metrics: TrackingMetrics::from_quality(TrackingQuality::new(50.0, 15.0, 0.95)),
            is_active: true,
        }
    }
}

impl TrackingHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_metrics(&mut self, metrics: TrackingMetrics) {
        self.current_metrics = metrics;
    }

    pub fn stop(&mut self) {
        self.is_active = false;
    }
}

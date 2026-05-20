// Pointing calibration — continuous pointing model maintenance
//
// Antenna pointing models drift continuously. Thermal expansion of the antenna
// structure throughout the day causes pointing errors. Wind loading shifts
// large dishes by arcminutes. Foundation settling shifts ground stations over
// years. The published pointing accuracy of any commercial antenna is the
// laboratory specification, not the operational reality.
//
// Real operators do continuous pointing calibration using known sources —
// radio stars, GPS satellites, or designated calibration satellites — and
// maintain pointing models that update every few hours based on observed
// versus expected positions.
//
// Without this, optical link acquisition success rates degrade from 95%+ to
// maybe 60-70% within months of installation. This module provides the
// infrastructure to maintain the pointing model continuously rather than
// assuming published antenna specs hold over time.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A known reference source used to calibrate the pointing model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalibrationSource {
    /// Natural radio source with known position. Flux density in Jy.
    /// Bright sources: Cassiopeia A (~2500 Jy at 1 GHz), Cygnus A (~1600 Jy).
    RadioStar {
        name: String,
        ra_hours: f64,
        dec_deg: f64,
        flux_jy: f64,
    },
    /// GPS satellite — always visible somewhere, precisely known ephemeris.
    GpsSatellite {
        prn: u8,
    },
    /// Dedicated calibration satellite from a bilateral operator agreement.
    CalibrationSatellite {
        norad_id: u64,
        operator: String,
    },
    /// Cooperative transponder beacon at a known fixed position on the ground.
    TransponderBeacon {
        beacon_id: String,
        lat_deg: f64,
        lon_deg: f64,
        alt_m: f64,
    },
}

impl CalibrationSource {
    pub fn name(&self) -> String {
        match self {
            Self::RadioStar { name, .. } => name.clone(),
            Self::GpsSatellite { prn } => format!("GPS-PRN{prn}"),
            Self::CalibrationSatellite { norad_id, .. } => format!("CalSat-{norad_id}"),
            Self::TransponderBeacon { beacon_id, .. } => format!("Beacon-{beacon_id}"),
        }
    }

    /// Whether this source is always available regardless of pass geometry.
    pub fn is_always_available(&self) -> bool {
        matches!(self, Self::TransponderBeacon { .. })
    }
}

/// A single pointing observation against a calibration source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationObservation {
    pub observed_at: DateTime<Utc>,
    pub source: CalibrationSource,
    /// Expected pointing from orbital mechanics / star catalog (degrees)
    pub expected_az_deg: f64,
    pub expected_el_deg: f64,
    /// Measured pointing from antenna encoder feedback (degrees)
    pub measured_az_deg: f64,
    pub measured_el_deg: f64,
    /// Signal quality (e.g., S/N of the beacon/star signal). Higher = more weight.
    pub signal_quality: f64,
}

impl CalibrationObservation {
    /// Az residual: measured − expected (milli-degrees)
    pub fn residual_az_mdeg(&self) -> f64 {
        (self.measured_az_deg - self.expected_az_deg) * 1000.0
    }

    /// El residual: measured − expected (milli-degrees)
    pub fn residual_el_mdeg(&self) -> f64 {
        (self.measured_el_deg - self.expected_el_deg) * 1000.0
    }

    /// Total residual magnitude (milli-degrees)
    pub fn residual_magnitude_mdeg(&self) -> f64 {
        let az = self.residual_az_mdeg();
        let el = self.residual_el_mdeg();
        (az * az + el * el).sqrt()
    }
}

/// Current best estimate of the pointing correction.
///
/// Applied as: commanded_pointing = nominal_model_pointing + correction
///
/// i.e., if the orbital model predicts the satellite at az=45°, el=30°, and
/// the correction is az_offset=+50 mdeg, el_offset=-30 mdeg, the antenna is
/// commanded to az=45.050°, el=29.970°.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PointingCorrection {
    /// Azimuth correction (milli-degrees)
    pub az_offset_mdeg: f64,
    /// Elevation correction (milli-degrees)
    pub el_offset_mdeg: f64,
    /// Confidence in this correction (0..1). Decays with time since last observation.
    pub confidence: f64,
    /// When this correction was last updated
    pub updated_at: DateTime<Utc>,
}

impl PointingCorrection {
    pub fn zero() -> Self {
        Self {
            az_offset_mdeg: 0.0,
            el_offset_mdeg: 0.0,
            confidence: 0.0,
            updated_at: Utc::now(),
        }
    }

    /// Apply this correction to a commanded az/el. Returns (corrected_az, corrected_el).
    pub fn apply(&self, az_deg: f64, el_deg: f64) -> (f64, f64) {
        (
            az_deg + self.az_offset_mdeg / 1000.0,
            el_deg + self.el_offset_mdeg / 1000.0,
        )
    }

    /// Whether this correction is fresh enough to trust for acquisition.
    pub fn is_fresh(&self, max_age: Duration) -> bool {
        let age = Utc::now() - self.updated_at;
        age < max_age && self.confidence > 0.3
    }

    /// Confidence with exponential decay applied for elapsed time.
    ///
    /// Thermal drift timescale: ~4 hours (full diurnal cycle).
    /// Confidence halves every 2 hours with no new observations.
    pub fn decayed_confidence(&self, now: DateTime<Utc>) -> f64 {
        let age_hours = (now - self.updated_at).num_seconds().max(0) as f64 / 3600.0;
        let half_life_hours = 2.0;
        let decay = 2.0_f64.powf(-age_hours / half_life_hours);
        (self.confidence * decay).max(0.0)
    }
}

/// Pointing model — rolling weighted average of recent calibration observations.
///
/// Weighted by:
///   - Observation signal quality (higher S/N → more weight)
///   - Recency (inverse-age weighting within the rolling window)
///
/// Outlier rejection: observations with residual > mean + 3σ of recent history
/// are discarded to protect against momentary interference or failed observations.
#[derive(Debug)]
pub struct PointingModel {
    history: VecDeque<CalibrationObservation>,
    max_history: usize,
    /// Rolling window for the weighted average (seconds)
    pub window_seconds: i64,
    current_correction: PointingCorrection,
}

impl PointingModel {
    pub fn new(window_seconds: i64) -> Self {
        Self {
            history: VecDeque::new(),
            max_history: 50,
            window_seconds,
            current_correction: PointingCorrection::zero(),
        }
    }

    /// Ingest a new calibration observation and update the pointing correction.
    pub fn update(&mut self, obs: CalibrationObservation) {
        if self.history.len() >= 3 && self.is_outlier(&obs) {
            tracing::warn!(
                "Rejecting outlier calibration observation from {}: residual {:.0} mdeg",
                obs.source.name(),
                obs.residual_magnitude_mdeg()
            );
            return;
        }

        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(obs);
        self.recompute_correction();
    }

    /// Apply the current pointing correction to commanded az/el.
    pub fn apply_correction(&self, az_deg: f64, el_deg: f64) -> (f64, f64) {
        self.current_correction.apply(az_deg, el_deg)
    }

    /// Current correction with live confidence decay applied.
    pub fn current_correction(&self) -> PointingCorrection {
        let now = Utc::now();
        let mut c = self.current_correction;
        c.confidence = c.decayed_confidence(now);
        c
    }

    /// Whether the pointing model is fresh enough for use in acquisition.
    pub fn is_operationally_current(&self) -> bool {
        self.current_correction.is_fresh(Duration::hours(4))
    }

    /// Number of observations in the active rolling window.
    pub fn active_observation_count(&self) -> usize {
        let cutoff = Utc::now() - Duration::seconds(self.window_seconds);
        self.history.iter().filter(|o| o.observed_at >= cutoff).count()
    }

    fn is_outlier(&self, candidate: &CalibrationObservation) -> bool {
        let residuals: Vec<f64> = self
            .history
            .iter()
            .map(|o| o.residual_magnitude_mdeg())
            .collect();
        let mean = residuals.iter().sum::<f64>() / residuals.len() as f64;
        let variance = residuals
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / residuals.len() as f64;
        let std_dev = variance.sqrt();
        candidate.residual_magnitude_mdeg() > mean + 3.0 * std_dev.max(10.0)
    }

    fn recompute_correction(&mut self) {
        let cutoff = Utc::now() - Duration::seconds(self.window_seconds);
        let recent: Vec<&CalibrationObservation> =
            self.history.iter().filter(|o| o.observed_at >= cutoff).collect();

        if recent.is_empty() {
            self.current_correction.confidence = 0.0;
            return;
        }

        let now = Utc::now();
        let mut total_weight = 0.0;
        let mut weighted_az = 0.0;
        let mut weighted_el = 0.0;

        for obs in &recent {
            let age_minutes = (now - obs.observed_at).num_seconds().max(0) as f64 / 60.0;
            let weight = obs.signal_quality / (age_minutes + 1.0);
            total_weight += weight;
            weighted_az += obs.residual_az_mdeg() * weight;
            weighted_el += obs.residual_el_mdeg() * weight;
        }

        if total_weight > 0.0 {
            self.current_correction = PointingCorrection {
                az_offset_mdeg: weighted_az / total_weight,
                el_offset_mdeg: weighted_el / total_weight,
                // Confidence grows with observation count, saturates at 5 observations
                confidence: (recent.len() as f64 / 5.0).min(1.0),
                updated_at: now,
            };
        }
    }
}

/// Calibration tracker — manages known reference sources and the pointing model.
///
/// In a full implementation this drives the antenna to slew to calibration
/// sources during gaps in mission traffic, records the pointing residuals,
/// and feeds them into the PointingModel. Here it maintains the model state
/// and provides the interface for injecting observations from any source.
pub struct CalibrationTracker {
    pub model: PointingModel,
    known_sources: Vec<CalibrationSource>,
    /// Minimum interval between two observations of the same source (seconds)
    pub min_observation_interval_s: i64,
    /// Target interval between any calibration observations (seconds)
    pub target_interval_s: i64,
    last_observation_at: Option<DateTime<Utc>>,
    observation_count: u64,
}

impl CalibrationTracker {
    pub fn new() -> Self {
        Self {
            model: PointingModel::new(4 * 3600), // 4-hour rolling window
            known_sources: Vec::new(),
            min_observation_interval_s: 300,  // 5 minutes minimum
            target_interval_s: 2 * 3600,      // 2-hour target
            last_observation_at: None,
            observation_count: 0,
        }
    }

    pub fn add_source(&mut self, source: CalibrationSource) {
        self.known_sources.push(source);
    }

    /// Ingest a pointing observation and update the model.
    pub fn record_observation(&mut self, obs: CalibrationObservation) {
        self.last_observation_at = Some(obs.observed_at);
        self.observation_count += 1;
        self.model.update(obs);

        let correction = self.model.current_correction();
        tracing::info!(
            "Pointing model updated: az_offset={:.0} mdeg, el_offset={:.0} mdeg, confidence={:.2}",
            correction.az_offset_mdeg,
            correction.el_offset_mdeg,
            correction.confidence,
        );
    }

    /// Whether a calibration observation is due based on the target interval.
    pub fn observation_due(&self) -> bool {
        match self.last_observation_at {
            None => true,
            Some(last) => (Utc::now() - last).num_seconds() >= self.target_interval_s,
        }
    }

    /// Apply the current pointing correction to commanded az/el.
    ///
    /// Falls back to uncorrected pointing if the model has no fresh data,
    /// logging a warning. This is the safe failure mode — uncorrected pointing
    /// matches published specs, but acquisition success rates will degrade over time.
    pub fn corrected_pointing(&self, az_deg: f64, el_deg: f64) -> (f64, f64) {
        if self.model.is_operationally_current() {
            self.model.apply_correction(az_deg, el_deg)
        } else {
            tracing::warn!(
                "Pointing model is stale (total observations: {}); using uncorrected pointing — acquisition success rate may be degraded",
                self.observation_count
            );
            (az_deg, el_deg)
        }
    }

    pub fn known_sources(&self) -> &[CalibrationSource] {
        &self.known_sources
    }

    pub fn observation_count(&self) -> u64 {
        self.observation_count
    }
}

impl Default for CalibrationTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_obs(
        az_residual_mdeg: f64,
        el_residual_mdeg: f64,
        quality: f64,
    ) -> CalibrationObservation {
        CalibrationObservation {
            observed_at: Utc::now(),
            source: CalibrationSource::GpsSatellite { prn: 5 },
            expected_az_deg: 180.0,
            expected_el_deg: 45.0,
            measured_az_deg: 180.0 + az_residual_mdeg / 1000.0,
            measured_el_deg: 45.0 + el_residual_mdeg / 1000.0,
            signal_quality: quality,
        }
    }

    #[test]
    fn test_model_converges_on_consistent_offset() {
        let mut model = PointingModel::new(3600);
        for _ in 0..5 {
            model.update(make_obs(50.0, 0.0, 1.0));
        }
        let correction = model.current_correction();
        assert!(
            (correction.az_offset_mdeg - 50.0).abs() < 5.0,
            "Should converge to ~50 mdeg, got {}",
            correction.az_offset_mdeg
        );
        assert!(correction.confidence > 0.5, "Should have meaningful confidence after 5 obs");
    }

    #[test]
    fn test_zero_correction_initially() {
        let model = PointingModel::new(3600);
        let c = model.current_correction();
        assert_eq!(c.az_offset_mdeg, 0.0);
        assert_eq!(c.el_offset_mdeg, 0.0);
        assert_eq!(c.confidence, 0.0);
    }

    #[test]
    fn test_tracker_corrects_pointing() {
        let mut tracker = CalibrationTracker::new();
        for _ in 0..5 {
            tracker.record_observation(make_obs(100.0, -50.0, 1.0));
        }
        let (az, el) = tracker.corrected_pointing(45.0, 30.0);
        // Correction ≈ +100 mdeg az, -50 mdeg el
        assert!((az - 45.1).abs() < 0.02, "az correction off: {az}");
        assert!((el - 29.95).abs() < 0.02, "el correction off: {el}");
    }

    #[test]
    fn test_outlier_rejected() {
        let mut model = PointingModel::new(3600);
        // Establish baseline: 10 mdeg offset
        for _ in 0..5 {
            model.update(make_obs(10.0, 0.0, 1.0));
        }
        let before = model.current_correction().az_offset_mdeg;
        // Inject a 500 mdeg outlier (>> 3σ of 10 mdeg history)
        model.update(make_obs(500.0, 0.0, 1.0));
        let after = model.current_correction().az_offset_mdeg;
        assert!(
            (after - before).abs() < 30.0,
            "Outlier should be rejected, correction shifted from {} to {}",
            before,
            after
        );
    }

    #[test]
    fn test_observation_due_when_no_history() {
        let tracker = CalibrationTracker::new();
        assert!(tracker.observation_due(), "Should be due with no history");
    }
}

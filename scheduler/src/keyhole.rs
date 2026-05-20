//! Keyhole geometry — az-el mount tracking limits near zenith
//!
//! Most ground stations can't track satellites that pass directly overhead.
//! The az-el mount geometry creates a zone near zenith where the azimuth
//! rotator can't slew fast enough to follow a high-elevation pass. Operators
//! call this the "keyhole."
//!
//! A satellite that passes at 85°+ elevation often loses 30–60 seconds of
//! contact in the middle of what should be the best part of the pass — when
//! the satellite is closest, range is shortest, and SNR is highest.
//!
//! This is why X-Y mounts exist (no keyhole) but they're more expensive and
//! rarer than az-el mounts. Most commercial ground stations have az-el and
//! live with the keyhole.
//!
//! The scheduler needs to know about each ground station's keyhole geometry
//! to either avoid high-elevation passes, apply an availability penalty to
//! them, or schedule a handoff to another station during the keyhole period.
//! This is the kind of detail that distinguishes professional ground station
//! software from amateur software.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Mount type — determines whether a zenith keyhole exists and where.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MountType {
    /// Azimuth-Elevation mount. The keyhole is the cone near zenith where the
    /// required az slew rate exceeds the rotator's mechanical limit.
    AzimuthElevation {
        /// Maximum az slew rate (degrees/second). Typical commercial: 3–10 deg/s.
        max_az_slew_deg_per_s: f64,
        /// Maximum el slew rate (degrees/second). Typical commercial: 3–8 deg/s.
        max_el_slew_deg_per_s: f64,
    },
    /// X-Y mount. No zenith keyhole — can track through overhead passes.
    /// More expensive; found at professional observatories and military facilities.
    XY,
    /// Equatorial/Parallactic mount. Keyhole at the celestial poles, not zenith.
    /// Rarely used for satellite tracking.
    Equatorial { max_ha_slew_deg_per_s: f64 },
}

impl MountType {
    /// Whether this mount type has a zenith keyhole.
    pub fn has_zenith_keyhole(&self) -> bool {
        matches!(self, Self::AzimuthElevation { .. })
    }
}

/// Keyhole zone configuration for an az-el mount.
///
/// The elevation threshold is derived from mount mechanics:
///   required_az_rate ≈ satellite_ground_speed / (range · cos(elevation))
/// As elevation → 90°, cos(elevation) → 0, so required_az_rate → ∞.
///
/// Threshold is the elevation above which the required az rate exceeds the
/// mount's rated maximum. For a typical 5 deg/s commercial mount and a LEO
/// satellite (orbital speed ~7 km/s), the keyhole starts around 75–85°.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyholeConfig {
    /// Elevation above which the mount enters the keyhole (degrees).
    /// Typical value: 75–85° for commercial az-el mounts (5 deg/s rated).
    pub threshold_elevation_deg: f64,
    /// Conservative estimate of contact loss when a pass enters the keyhole (seconds).
    /// Actual loss depends on pass geometry and mount response; this is used for
    /// scheduling penalty calculation without full ephemeris access.
    pub typical_loss_duration_s: u32,
    /// Whether the scheduler should attempt to arrange a handoff to another
    /// station to cover the keyhole gap.
    pub schedule_handoff: bool,
}

impl KeyholeConfig {
    /// Default for a typical commercial az-el mount (~5 deg/s az slew rate).
    pub fn default_azel() -> Self {
        Self {
            threshold_elevation_deg: 80.0,
            typical_loss_duration_s: 45,
            schedule_handoff: true,
        }
    }

    /// High-speed mount with faster rotators — tighter keyhole zone.
    pub fn high_speed_azel() -> Self {
        Self {
            threshold_elevation_deg: 87.0,
            typical_loss_duration_s: 15,
            schedule_handoff: false,
        }
    }

    /// Slow or older mount with limited slew rate — wider keyhole.
    pub fn slow_azel() -> Self {
        Self {
            threshold_elevation_deg: 75.0,
            typical_loss_duration_s: 60,
            schedule_handoff: true,
        }
    }
}

/// Result of analysing a satellite pass against a ground station's keyhole geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyholeAnalysis {
    pub station_id: String,
    /// Maximum elevation this pass reaches (degrees)
    pub pass_max_elevation_deg: f64,
    /// Whether any portion of the pass enters the keyhole zone
    pub enters_keyhole: bool,
    /// Time at which the pass enters the keyhole (if any)
    pub keyhole_entry: Option<DateTime<Utc>>,
    /// Time at which the pass exits the keyhole (if any)
    pub keyhole_exit: Option<DateTime<Utc>>,
    /// Estimated contact loss due to keyhole (seconds)
    pub estimated_loss_s: u32,
    /// Whether a handoff to another station is advisable
    pub handoff_recommended: bool,
    /// Fraction of the pass duration lost to keyhole (0..1)
    pub loss_fraction: f64,
}

impl KeyholeAnalysis {
    /// Whether this pass is materially degraded by the keyhole.
    pub fn is_significant(&self) -> bool {
        self.estimated_loss_s > 10 && self.loss_fraction > 0.05
    }
}

/// Ground station mount geometry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundStationGeometry {
    pub station_id: String,
    pub mount_type: MountType,
    /// Keyhole config — only relevant for AzimuthElevation mounts.
    pub keyhole: Option<KeyholeConfig>,
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub altitude_m: f64,
}

impl GroundStationGeometry {
    pub fn new_azel(
        station_id: String,
        lat_deg: f64,
        lon_deg: f64,
        alt_m: f64,
        keyhole: KeyholeConfig,
    ) -> Self {
        Self {
            station_id,
            mount_type: MountType::AzimuthElevation {
                max_az_slew_deg_per_s: 5.0,
                max_el_slew_deg_per_s: 5.0,
            },
            keyhole: Some(keyhole),
            latitude_deg: lat_deg,
            longitude_deg: lon_deg,
            altitude_m: alt_m,
        }
    }

    pub fn new_xy(station_id: String, lat_deg: f64, lon_deg: f64, alt_m: f64) -> Self {
        Self {
            station_id,
            mount_type: MountType::XY,
            keyhole: None,
            latitude_deg: lat_deg,
            longitude_deg: lon_deg,
            altitude_m: alt_m,
        }
    }

    /// Whether a given elevation is inside the keyhole zone.
    pub fn in_keyhole(&self, elevation_deg: f64) -> bool {
        match &self.keyhole {
            Some(kh) if self.mount_type.has_zenith_keyhole() => {
                elevation_deg >= kh.threshold_elevation_deg
            }
            _ => false,
        }
    }
}

/// Analyse a satellite pass elevation profile against a ground station's keyhole geometry.
///
/// `profile` is a time-ordered sequence of (timestamp, elevation_deg, azimuth_deg)
/// sampled at any interval (typically 1–10 seconds from an ephemeris propagator).
///
/// `pass_duration_s` is the total pass duration in seconds (used to compute
/// `loss_fraction` without assuming uniform sampling in `profile`).
pub fn analyse_pass(
    station: &GroundStationGeometry,
    profile: &[(DateTime<Utc>, f64, f64)],
    pass_duration_s: u32,
) -> KeyholeAnalysis {
    let max_el = profile
        .iter()
        .map(|(_, el, _)| *el)
        .fold(f64::NEG_INFINITY, f64::max);

    let kh_config = match &station.keyhole {
        Some(kh) if station.mount_type.has_zenith_keyhole() => kh,
        _ => {
            return KeyholeAnalysis {
                station_id: station.station_id.clone(),
                pass_max_elevation_deg: max_el,
                enters_keyhole: false,
                keyhole_entry: None,
                keyhole_exit: None,
                estimated_loss_s: 0,
                handoff_recommended: false,
                loss_fraction: 0.0,
            };
        }
    };

    let threshold = kh_config.threshold_elevation_deg;
    let mut entry_time: Option<DateTime<Utc>> = None;
    let mut exit_time: Option<DateTime<Utc>> = None;
    let mut keyhole_seconds = 0i64;

    for window in profile.windows(2) {
        let (t1, el1, _) = window[0];
        let (t2, el2, _) = window[1];
        let step_s = (t2 - t1).num_seconds();

        let in1 = el1 >= threshold;
        let in2 = el2 >= threshold;

        if in1 || in2 {
            keyhole_seconds += step_s;
        }
        if !in1 && in2 && entry_time.is_none() {
            entry_time = Some(t1);
        }
        if in1 && !in2 {
            exit_time = Some(t2);
        }
    }

    let enters = keyhole_seconds > 0;
    let estimated_loss = if enters {
        keyhole_seconds.min(kh_config.typical_loss_duration_s as i64) as u32
    } else {
        0
    };
    let loss_fraction = if pass_duration_s > 0 {
        estimated_loss as f64 / pass_duration_s as f64
    } else {
        0.0
    };

    KeyholeAnalysis {
        station_id: station.station_id.clone(),
        pass_max_elevation_deg: max_el,
        enters_keyhole: enters,
        keyhole_entry: entry_time,
        keyhole_exit: exit_time,
        estimated_loss_s: estimated_loss,
        handoff_recommended: enters && kh_config.schedule_handoff,
        loss_fraction,
    }
}

/// Score penalty for a pass that enters the keyhole zone.
///
/// Returns a negative value to subtract from the schedule score in the
/// simulated annealing optimizer. The penalty accounts for whether a
/// handoff to another station is available to cover the gap.
///
/// No handoff: full loss_fraction penalty (contact is simply gone).
/// With handoff: 30% of the penalty (gap is covered, but coordination overhead).
pub fn keyhole_score_penalty(analysis: &KeyholeAnalysis, handoff_available: bool) -> f64 {
    if !analysis.enters_keyhole {
        return 0.0;
    }
    let base_penalty = analysis.loss_fraction * 100.0;
    if handoff_available {
        -base_penalty * 0.3
    } else {
        -base_penalty
    }
}

/// Fast heuristic: does a pass's max elevation suggest it will enter the keyhole?
///
/// Use this when a full elevation profile is unavailable at schedule time.
/// The scheduler can use this to flag passes for handoff planning without
/// running a full propagator for every candidate pass.
pub fn is_likely_keyhole_pass(max_elevation_deg: f64, config: &KeyholeConfig) -> bool {
    max_elevation_deg >= config.threshold_elevation_deg
}

/// A keyhole warning attached to a scheduled pass.
///
/// The scheduler embeds this in `ScheduledPass` when a pass is known to
/// enter the keyhole zone, allowing downstream systems (pass executor,
/// operations team, handoff coordinator) to plan accordingly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyholeWarning {
    /// Estimated contact loss (seconds)
    pub estimated_loss_s: u32,
    /// Whether a handoff has been scheduled to cover the gap
    pub handoff_scheduled: bool,
    /// Maximum elevation of this pass (degrees)
    pub max_elevation_deg: f64,
    /// Fraction of the pass duration affected (0..1)
    pub loss_fraction: f64,
}

impl KeyholeWarning {
    pub fn from_analysis(analysis: &KeyholeAnalysis) -> Self {
        Self {
            estimated_loss_s: analysis.estimated_loss_s,
            handoff_scheduled: false,
            max_elevation_deg: analysis.pass_max_elevation_deg,
            loss_fraction: analysis.loss_fraction,
        }
    }

    pub fn mark_handoff_scheduled(&mut self) {
        self.handoff_scheduled = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(elevations: &[f64]) -> Vec<(DateTime<Utc>, f64, f64)> {
        let start = Utc::now();
        elevations
            .iter()
            .enumerate()
            .map(|(i, &el)| {
                (
                    start + Duration::seconds(i as i64 * 10),
                    el,
                    180.0, // azimuth (south — doesn't affect keyhole detection)
                )
            })
            .collect()
    }

    #[test]
    fn test_no_keyhole_low_elevation_pass() {
        let station = GroundStationGeometry::new_azel(
            "test-station".to_string(),
            43.0,
            -79.0,
            100.0,
            KeyholeConfig::default_azel(),
        );
        let profile = make_profile(&[5.0, 20.0, 35.0, 45.0, 35.0, 20.0, 5.0]);
        let analysis = analyse_pass(&station, &profile, 70);
        assert!(!analysis.enters_keyhole);
        assert_eq!(analysis.estimated_loss_s, 0);
        assert!(!analysis.handoff_recommended);
    }

    #[test]
    fn test_keyhole_high_elevation_pass() {
        let station = GroundStationGeometry::new_azel(
            "test-station".to_string(),
            43.0,
            -79.0,
            100.0,
            KeyholeConfig::default_azel(), // threshold = 80°
        );
        let profile = make_profile(&[10.0, 30.0, 60.0, 82.0, 85.0, 82.0, 60.0, 30.0, 10.0]);
        let analysis = analyse_pass(&station, &profile, 90);
        assert!(analysis.enters_keyhole);
        assert!(analysis.estimated_loss_s > 0);
        assert!(analysis.handoff_recommended);
        assert!(analysis.loss_fraction > 0.0);
    }

    #[test]
    fn test_xy_mount_no_keyhole_even_overhead() {
        let station = GroundStationGeometry::new_xy("xy-station".to_string(), 43.0, -79.0, 100.0);
        let profile = make_profile(&[10.0, 45.0, 85.0, 89.0, 89.0, 85.0, 45.0, 10.0]);
        let analysis = analyse_pass(&station, &profile, 80);
        assert!(!analysis.enters_keyhole, "X-Y mount should have no keyhole");
        assert_eq!(analysis.estimated_loss_s, 0);
    }

    #[test]
    fn test_penalty_no_handoff_worse_than_with_handoff() {
        let analysis = KeyholeAnalysis {
            station_id: "test".to_string(),
            pass_max_elevation_deg: 85.0,
            enters_keyhole: true,
            keyhole_entry: None,
            keyhole_exit: None,
            estimated_loss_s: 30,
            handoff_recommended: true,
            loss_fraction: 0.2,
        };
        let p_no_handoff = keyhole_score_penalty(&analysis, false);
        let p_with_handoff = keyhole_score_penalty(&analysis, true);
        assert!(
            p_no_handoff < p_with_handoff,
            "No handoff must be penalised more"
        );
        assert!(
            p_no_handoff < 0.0,
            "Penalty must be negative"
        );
        assert!(
            p_with_handoff < 0.0,
            "Even covered keyhole has some penalty"
        );
    }

    #[test]
    fn test_no_penalty_for_clean_pass() {
        let analysis = KeyholeAnalysis {
            station_id: "test".to_string(),
            pass_max_elevation_deg: 45.0,
            enters_keyhole: false,
            keyhole_entry: None,
            keyhole_exit: None,
            estimated_loss_s: 0,
            handoff_recommended: false,
            loss_fraction: 0.0,
        };
        assert_eq!(keyhole_score_penalty(&analysis, false), 0.0);
        assert_eq!(keyhole_score_penalty(&analysis, true), 0.0);
    }

    #[test]
    fn test_heuristic_flag() {
        let config = KeyholeConfig::default_azel(); // threshold = 80°
        assert!(is_likely_keyhole_pass(82.0, &config));
        assert!(!is_likely_keyhole_pass(70.0, &config));
    }
}

//! Predictive NCO programming with phase-continuous Doppler correction
//!
//! This implements Problem 2: Doppler correction without sample loss during retuning.
//! Instead of reactive retuning (which introduces settling time), we program the NCO
//! with a frequency schedule derived from orbital prediction.

use crate::sdr::SampleId;
use chrono::{DateTime, Utc};
use ground_core::{Frequency, Result};
use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements, MinutesSinceEpoch};
use tracing;

/// Frequency offset from base frequency in Hz
pub type FrequencyOffset = i64;

/// Doppler schedule for a pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopplerSchedule {
    /// Base frequency (carrier frequency before Doppler)
    pub base_frequency: Frequency,
    /// Sample ID to frequency offset mapping
    /// Stored as a sorted list for efficient interpolation
    pub samples: Vec<(SampleId, FrequencyOffset)>,
    /// When this schedule was computed
    pub computed_at: DateTime<Utc>,
    /// TLE epoch used for computation
    pub tle_epoch: DateTime<Utc>,
}

impl DopplerSchedule {
    /// Create a new Doppler schedule from orbital prediction
    pub fn from_orbital_prediction(
        base_frequency: Frequency,
        sample_rate_hz: u64,
        predictions: &[(DateTime<Utc>, FrequencyOffset)],
        start_sample_id: SampleId,
    ) -> Self {
        let samples = predictions
            .iter()
            .enumerate()
            .map(|(i, (_, offset))| {
                let sample_id =
                    SampleId::new(start_sample_id.as_u64() + (i as u64 * sample_rate_hz / 1000));
                (sample_id, *offset)
            })
            .collect();

        Self {
            base_frequency,
            samples,
            computed_at: Utc::now(),
            tle_epoch: predictions
                .first()
                .map(|(t, _)| *t)
                .unwrap_or_else(Utc::now),
        }
    }

    /// Interpolate the frequency offset for a given sample ID
    pub fn interpolate(&self, sample_id: SampleId) -> FrequencyOffset {
        // Binary search for the surrounding samples
        let idx = match self.samples.binary_search_by_key(&sample_id, |(id, _)| *id) {
            Ok(i) => return self.samples[i].1,
            Err(i) => i,
        };

        if idx == 0 {
            return self.samples.first().map(|(_, offset)| *offset).unwrap_or(0);
        }

        if idx >= self.samples.len() {
            return self.samples.last().map(|(_, offset)| *offset).unwrap_or(0);
        }

        // Linear interpolation between the two surrounding samples
        let (id1, offset1) = self.samples[idx - 1];
        let (id2, offset2) = self.samples[idx];

        let t = (sample_id.as_u64() - id1.as_u64()) as f64 / (id2.as_u64() - id1.as_u64()) as f64;
        (offset1 as f64 + t * (offset2 as f64 - offset1 as f64)) as i64
    }
}

/// Numerically-Controlled Oscillator with phase-continuous frequency updates
pub struct NcoController {
    /// Current phase accumulator (0.0 to 1.0)
    phase: f64,
    /// Current frequency offset in Hz
    current_offset: FrequencyOffset,
    /// Sample rate in Hz
    sample_rate: u64,
}

impl NcoController {
    pub fn new(sample_rate: u64) -> Self {
        Self {
            phase: 0.0,
            current_offset: 0,
            sample_rate,
        }
    }

    /// Set frequency offset with phase continuity
    /// The NCO accumulates phase across frequency changes rather than resetting
    pub fn set_frequency_continuous(&mut self, offset: FrequencyOffset) {
        self.current_offset = offset;
        // Phase is NOT reset - this is the key to phase continuity
    }

    /// Get the current frequency offset
    pub fn current_offset(&self) -> FrequencyOffset {
        self.current_offset
    }

    /// Process a single sample through the NCO
    /// Returns the phase-adjusted frequency for this sample
    pub fn process_sample(&mut self) -> f64 {
        // Update phase accumulator
        let phase_increment = (self.current_offset as f64) / (self.sample_rate as f64);
        self.phase = (self.phase + phase_increment) % 1.0;

        // Return the current phase (used for mixing)
        self.phase
    }

    /// Reset phase accumulator
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }
}

/// Apply Doppler correction to a sample stream using a schedule
pub fn apply_doppler_schedule(
    nco: &mut NcoController,
    schedule: &DopplerSchedule,
    current_sample: SampleId,
) -> Result<Frequency> {
    let target_offset = schedule.interpolate(current_sample);
    nco.set_frequency_continuous(target_offset);

    let corrected_frequency = (schedule.base_frequency as i64 + target_offset) as u64;
    Ok(corrected_frequency)
}

/// Doppler predictor using SGP4 propagation
pub struct DopplerPredictor {
    /// SGP4 constants for the satellite
    constants: Option<Constants>,
    /// TLE epoch used when the constants were loaded (required for MinutesSinceEpoch)
    tle_epoch: Option<DateTime<Utc>>,
    /// Station geodetic coordinates (lat, lon, alt in km)
    #[allow(dead_code)]
    station: (f64, f64, f64),
    /// Carrier frequency in Hz (for Doppler conversion)
    carrier_frequency: f64,
    /// UKF refiner for orbital state refinement (optional, enables feedback loop)
    ukf_refiner: Option<UkfRefiner>,
}

impl DopplerPredictor {
    pub fn new(
        station_lat_deg: f64,
        station_lon_deg: f64,
        station_alt_km: f64,
        carrier_frequency: f64,
    ) -> Self {
        Self {
            constants: None,
            tle_epoch: None,
            station: (station_lat_deg, station_lon_deg, station_alt_km),
            carrier_frequency,
            ukf_refiner: None,
        }
    }

    /// Enable UKF-based orbital refinement for Doppler predictions
    pub fn with_ukf_refiner(mut self, ukf_refiner: UkfRefiner) -> Self {
        self.ukf_refiner = Some(ukf_refiner);
        self
    }

    /// Load TLE and initialize the SGP4 constants
    pub fn load_tle(&mut self, tle_line1: &str, tle_line2: &str) -> Result<()> {
        let elements = Elements::from_tle(
            Some("satellite".to_string()),
            tle_line1.as_bytes(),
            tle_line2.as_bytes(),
        )
        .map_err(|e| ground_core::GroundStationError::Hardware(format!("Invalid TLE: {}", e)))?;

        // Store the TLE epoch before elements is borrowed by Constants::from_elements.
        // SGP4's MinutesSinceEpoch is relative to this epoch, not the Unix epoch.
        self.tle_epoch = Some(elements.datetime.and_utc());

        let constants = Constants::from_elements(&elements).map_err(|e| {
            ground_core::GroundStationError::Hardware(format!("SGP4 constants error: {}", e))
        })?;

        self.constants = Some(constants);
        Ok(())
    }

    /// Predict Doppler shift for a time window using SGP4 propagation
    #[allow(clippy::too_many_arguments)]
    pub fn predict_doppler(
        &self,
        satellite_id: &str,
        station_lat: f64,
        station_lon: f64,
        station_alt: f64,
        start_time: DateTime<Utc>,
        duration_sec: u64,
        step_ms: u64,
    ) -> Vec<(DateTime<Utc>, FrequencyOffset)> {
        let Some(constants) = &self.constants else {
            tracing::warn!(
                "No TLE loaded for satellite {}, returning empty Doppler prediction",
                satellite_id
            );
            return vec![];
        };

        const C: f64 = 299_792.458; // Speed of light in km/s
        let mut predictions = Vec::new();
        let mut current_time = start_time;
        let steps = (duration_sec * 1000 / step_ms) as usize;
        let step_sec = step_ms as f64 / 1000.0;

        let tle_epoch = match self.tle_epoch {
            Some(e) => e,
            None => {
                tracing::warn!("TLE epoch not available; cannot compute minutes-since-epoch");
                return vec![];
            }
        };

        for _ in 0..steps {
            let minutes_since_epoch = current_time
                .signed_duration_since(tle_epoch)
                .num_milliseconds() as f64
                / 60_000.0;

            let position = match constants.propagate(MinutesSinceEpoch(minutes_since_epoch)) {
                Ok(pos) => pos,
                Err(e) => {
                    tracing::warn!("SGP4 propagation error at {:?}: {}", current_time, e);
                    break;
                }
            };

            // Convert station geodetic to ECI
            let station_eci =
                self.geodetic_to_eci(station_lat, station_lon, station_alt, current_time);

            // Compute range vector from station to satellite
            let rx = position.position[0] - station_eci[0];
            let ry = position.position[1] - station_eci[1];
            let rz = position.position[2] - station_eci[2];
            let range = (rx * rx + ry * ry + rz * rz).sqrt();

            // Compute range rate by finite difference (look ahead by step_sec)
            let future_time = current_time + chrono::Duration::milliseconds(step_ms as i64);
            let future_minutes = future_time
                .signed_duration_since(tle_epoch)
                .num_milliseconds() as f64
                / 60_000.0;
            let future_position = match constants.propagate(MinutesSinceEpoch(future_minutes)) {
                Ok(pos) => pos,
                Err(_) => {
                    predictions.push((current_time, 0));
                    current_time = future_time;
                    continue;
                }
            };
            let future_station_eci =
                self.geodetic_to_eci(station_lat, station_lon, station_alt, future_time);
            let future_rx = future_position.position[0] - future_station_eci[0];
            let future_ry = future_position.position[1] - future_station_eci[1];
            let future_rz = future_position.position[2] - future_station_eci[2];
            let future_range =
                (future_rx * future_rx + future_ry * future_ry + future_rz * future_rz).sqrt();

            let range_rate = (future_range - range) / step_sec; // km/s

            // Doppler shift: f_doppler = f_carrier * (drange/dt) / c
            let doppler = self.carrier_frequency * (range_rate / C);

            predictions.push((current_time, doppler as i64));
            current_time = future_time;
        }

        predictions
    }

    /// Convert geodetic coordinates (lat, lon in degrees, alt in km) to ECI at a given time.
    /// Uses IAU 1982 GMST and WGS84 prime-vertical-radius formula (matches tracking::ukf).
    fn geodetic_to_eci(
        &self,
        lat_deg: f64,
        lon_deg: f64,
        alt_km: f64,
        time: DateTime<Utc>,
    ) -> [f64; 3] {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();

        // WGS84 constants (km)
        const RE: f64 = 6_378.137;
        const FLATTENING: f64 = 1.0 / 298.257_223_563;

        // Prime-vertical radius of curvature N(lat)
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        let n = RE / (1.0 - FLATTENING * (2.0 - FLATTENING) * sin_lat * sin_lat).sqrt();

        // IAU 1982 GMST — same formula used in tracking/src/ukf.rs
        // jd: Julian date; t_ut1: Julian centuries from J2000.0
        let jd = 2_451_545.0 + (time.timestamp() as f64 - 946_727_935.816) / 86_400.0;
        let t_ut1 = (jd - 2_451_545.0) / 36_525.0;
        let gmst_sec =
            67_310.548_41 + (8_640_184.812_866 + (0.093_104 - 6.2e-6 * t_ut1) * t_ut1) * t_ut1;
        let gmst_rad =
            (gmst_sec * std::f64::consts::PI / 43_200.0).rem_euclid(2.0 * std::f64::consts::PI);

        // Local Sidereal Time
        let lst = (gmst_rad + lon).rem_euclid(2.0 * std::f64::consts::PI);

        // ECI position (km) — standard WGS84 geodetic-to-ECEF then rotate by GMST
        let x = (n + alt_km) * cos_lat * lst.cos();
        let y = (n + alt_km) * cos_lat * lst.sin();
        let z = (n * (1.0 - FLATTENING).powi(2) + alt_km) * sin_lat;

        [x, y, z]
    }

    /// Refine propagation using observed Doppler residuals (UKF)
    /// This addresses the SGP4 accuracy limitation mentioned in Problem 2
    pub fn refine_with_observation(
        &mut self,
        observed_doppler: FrequencyOffset,
        predicted_doppler: FrequencyOffset,
        observation_time: DateTime<Utc>,
    ) {
        let residual = observed_doppler - predicted_doppler;
        tracing::debug!(
            "Doppler residual at {:?}: {} Hz",
            observation_time,
            residual
        );

        // If UKF refiner is available, use it to refine the orbital state
        if let Some(refiner) = &mut self.ukf_refiner {
            // Convert station coordinates from km to m for UKF
            let station_lat = self.station.0;
            let station_lon = self.station.1;
            let station_alt_m = self.station.2 * 1000.0;

            // Initialize UKF with current SGP4 state if not already done
            if refiner.states.is_empty() {
                if let Some(constants) = &self.constants {
                    // Get current SGP4 state at observation time
                    if let Some(tle_epoch) = self.tle_epoch {
                        let minutes_since =
                            (observation_time - tle_epoch).num_seconds() as f64 / 60.0;
                        if let Ok(state) = constants.propagate(MinutesSinceEpoch(minutes_since)) {
                            let orbital_state = OrbitalState {
                                position: nalgebra::Vector3::new(
                                    state.position[0],
                                    state.position[1],
                                    state.position[2],
                                ),
                                velocity: nalgebra::Vector3::new(
                                    state.velocity[0],
                                    state.velocity[1],
                                    state.velocity[2],
                                ),
                                time: observation_time,
                                tle_epoch,
                            };
                            refiner.initialize("current".to_string(), &orbital_state);
                        }
                    }
                }
            }

            // Feed the observation to the UKF
            if let Err(e) = refiner.refine_with_observation(
                "current",
                observed_doppler as Frequency,
                predicted_doppler as Frequency,
                observation_time,
            ) {
                tracing::warn!("UKF refinement failed: {}", e);
            } else {
                tracing::debug!("UKF refined orbital state using Doppler observation");
            }
        }
    }
}

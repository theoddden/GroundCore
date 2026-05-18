//! Unscented Kalman Filter for orbital state refinement
//!
//! Implements a proper sigma-point UKF using:
//! - J2-perturbed orbital dynamics (RK4) for the predict step
//! - GMST-rotated station ECI for accurate Earth-rotation-aware geometry
//! - Velocity-aware Doppler measurement model: h(x) = f_c*(r̂·v_rel)/c
//! - 2n+1 = 13 sigma points via scaled unscented transform (α=0.1, β=2, κ=0)

use crate::propagation::OrbitalState;
use chrono::{DateTime, Utc};
use ground_core::{Frequency, Result};
use nalgebra::{Matrix6, Vector3, Vector6};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

// ── Physical constants ────────────────────────────────────────────────────────
const MU: f64 = 3.986_004_418e14; // Earth gravitational parameter (m³/s²)
const R_EARTH: f64 = 6_378_137.0; // Earth equatorial radius (m)
const J2: f64 = 1.082_63e-3; // Earth's J2 oblateness coefficient
const OMEGA_EARTH: f64 = 7.292_115_0e-5; // Earth rotation rate (rad/s)
const SPEED_OF_LIGHT: f64 = 299_792_458.0; // m/s

// ── UKF tuning parameters ─────────────────────────────────────────────────────
const N: usize = 6; // State dimension
const ALPHA: f64 = 0.1; // Spread of sigma points around mean
const BETA: f64 = 2.0; // Prior knowledge parameter (optimal for Gaussian)
const KAPPA: f64 = 0.0; // Secondary scaling parameter
const LAMBDA: f64 = ALPHA * ALPHA * (N as f64 + KAPPA) - N as f64; // ≈ -5.94

// Sigma-point weights
fn weights_mean() -> [f64; 2 * N + 1] {
    let w0 = LAMBDA / (N as f64 + LAMBDA);
    let wi = 0.5 / (N as f64 + LAMBDA);
    let mut w = [wi; 2 * N + 1];
    w[0] = w0;
    w
}

fn weights_cov() -> [f64; 2 * N + 1] {
    let w0 = LAMBDA / (N as f64 + LAMBDA) + (1.0 - ALPHA * ALPHA + BETA);
    let wi = 0.5 / (N as f64 + LAMBDA);
    let mut w = [wi; 2 * N + 1];
    w[0] = w0;
    w
}

// ── J2-perturbed orbital dynamics ─────────────────────────────────────────────

/// Compute gravitational + J2 acceleration for state [x, y, z, vx, vy, vz]
fn orbital_dynamics(state: &Vector6<f64>) -> Vector6<f64> {
    let r = Vector3::new(state[0], state[1], state[2]);
    let v = Vector3::new(state[3], state[4], state[5]);
    let r_norm = r.norm();
    let r2 = r_norm * r_norm;
    let r5 = r2 * r2 * r_norm;

    // Two-body gravity
    let a_grav = -MU / (r_norm * r2) * r;

    // J2 perturbation
    let z2_over_r2 = (state[2] * state[2]) / r2;
    let j2_factor = 1.5 * J2 * MU * R_EARTH * R_EARTH / r5;
    let a_j2 = Vector3::new(
        j2_factor * state[0] * (5.0 * z2_over_r2 - 1.0),
        j2_factor * state[1] * (5.0 * z2_over_r2 - 1.0),
        j2_factor * state[2] * (5.0 * z2_over_r2 - 3.0),
    );

    let a = a_grav + a_j2;
    Vector6::new(v[0], v[1], v[2], a[0], a[1], a[2])
}

/// Integrate orbital state forward by `dt` seconds using RK4
fn rk4_propagate(state: &Vector6<f64>, dt: f64) -> Vector6<f64> {
    let k1 = orbital_dynamics(state);
    let k2 = orbital_dynamics(&(state + k1 * (dt / 2.0)));
    let k3 = orbital_dynamics(&(state + k2 * (dt / 2.0)));
    let k4 = orbital_dynamics(&(state + k3 * dt));
    state + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0)
}

// ── GMST rotation ─────────────────────────────────────────────────────────────

/// Greenwich Mean Sidereal Time for a given UTC instant (radians).
/// Uses IAU 1982 mean sidereal time formula.
fn gmst_radians(t: DateTime<Utc>) -> f64 {
    // Julian date of J2000.0 epoch = 2451545.0
    let jd = 2_451_545.0 + (t.timestamp() as f64 - 946_727_935.816) / 86_400.0;
    let t_ut1 = (jd - 2_451_545.0) / 36_525.0;
    // GMST in seconds
    let gmst_sec =
        67_310.548_41 + (8_640_184.812_866 + (0.093_104 - 6.2e-6 * t_ut1) * t_ut1) * t_ut1;
    (gmst_sec * PI / 43_200.0).rem_euclid(2.0 * PI)
}

/// Convert geodetic station coordinates to ECI at time `t`.
/// `lat_deg` / `lon_deg` are geographic (geodetic) degrees, `alt_m` is altitude in metres.
pub fn station_to_eci(lat_deg: f64, lon_deg: f64, alt_m: f64, t: DateTime<Utc>) -> Vector3<f64> {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let gmst = gmst_radians(t);
    let local_sidereal = (gmst + lon).rem_euclid(2.0 * PI);
    let r = R_EARTH + alt_m;
    Vector3::new(
        r * lat.cos() * local_sidereal.cos(),
        r * lat.cos() * local_sidereal.sin(),
        r * lat.sin(),
    )
}

/// Station velocity in ECI due to Earth's rotation: v = ω_earth × r_station
fn station_velocity_eci(station_eci: &Vector3<f64>) -> Vector3<f64> {
    Vector3::new(
        -OMEGA_EARTH * station_eci[1],
        OMEGA_EARTH * station_eci[0],
        0.0,
    )
}

// ── Doppler measurement model ─────────────────────────────────────────────────

/// Doppler shift (Hz) predicted for a satellite state and carrier frequency.
/// h(x) = f_carrier * (r̂ · v_rel) / c  where r̂ is the unit range vector
/// and v_rel = v_sat − v_station_eci accounts for station velocity.
pub fn doppler_measurement(
    sat_state: &Vector6<f64>,
    station_eci: &Vector3<f64>,
    carrier_hz: f64,
) -> f64 {
    let r_sat = Vector3::new(sat_state[0], sat_state[1], sat_state[2]);
    let v_sat = Vector3::new(sat_state[3], sat_state[4], sat_state[5]);
    let v_station = station_velocity_eci(station_eci);

    let r_rel = r_sat - station_eci;
    let r_norm = r_rel.norm();
    if r_norm < 1.0 {
        return 0.0;
    }
    let r_hat = r_rel / r_norm;
    let v_rel = v_sat - v_station;
    // Positive Doppler when satellite approaches
    -carrier_hz * r_hat.dot(&v_rel) / SPEED_OF_LIGHT
}

// ── UKF state ─────────────────────────────────────────────────────────────────

/// UKF state for orbital refinement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UkfState {
    /// State vector: [x, y, z, vx, vy, vz] in ECI (metres, m/s)
    pub state: Vector6<f64>,
    /// State covariance matrix
    pub covariance: Matrix6<f64>,
    /// Process noise covariance (applied per-second in predict)
    pub process_noise: Matrix6<f64>,
    /// Doppler measurement noise variance (Hz²)
    pub measurement_noise: f64,
    /// When this state is valid
    pub time: DateTime<Utc>,
}

impl UkfState {
    /// Create new UKF state from orbital state with sensible initial uncertainty
    pub fn from_orbital_state(state: &OrbitalState) -> Self {
        let state_vec = Vector6::new(
            state.position[0],
            state.position[1],
            state.position[2],
            state.velocity[0],
            state.velocity[1],
            state.velocity[2],
        );

        let covariance = Matrix6::from_diagonal(&Vector6::new(
            1_000_000.0, // 1 km position uncertainty (m²)
            1_000_000.0,
            1_000_000.0,
            100.0, // 10 m/s velocity uncertainty (m²/s²)
            100.0,
            100.0,
        ));

        let process_noise = Matrix6::from_diagonal(&Vector6::new(
            1.0, // m²/s residual position noise per second
            1.0, 1.0, 1e-4, // m²/s³ velocity noise per second
            1e-4, 1e-4,
        ));

        Self {
            state: state_vec,
            covariance,
            process_noise,
            measurement_noise: 100.0, // 10 Hz std-dev Doppler noise
            time: state.time,
        }
    }

    // ── Sigma-point generation ────────────────────────────────────────────────

    /// Generate 2n+1 = 13 sigma points using Cholesky decomposition of
    /// (n + λ) * P.  Falls back to identity-scaled spread if P is not PD.
    fn sigma_points(&self) -> [Vector6<f64>; 2 * N + 1] {
        let scale = N as f64 + LAMBDA;
        let scaled_p = self.covariance * scale;

        // Cholesky: scaled_p = L * L^T
        let l = scaled_p.cholesky().map(|c| c.l()).unwrap_or_else(|| {
            // Regularise with a small diagonal and retry
            (scaled_p + Matrix6::identity() * 1e-6)
                .cholesky()
                .map(|c| c.l())
                .unwrap_or_else(|| Matrix6::identity() * scale.sqrt())
        });

        let mut sigma = [Vector6::zeros(); 2 * N + 1];
        sigma[0] = self.state;
        for i in 0..N {
            let col = l.column(i).into_owned();
            sigma[i + 1] = self.state + col;
            sigma[N + i + 1] = self.state - col;
        }
        sigma
    }

    // ── Predict step ──────────────────────────────────────────────────────────

    /// Propagate state forward by `dt_sec` using J2-perturbed RK4 dynamics and
    /// the unscented transform to update the covariance.
    pub fn predict(&mut self, dt_sec: f64) {
        let wm = weights_mean();
        let wc = weights_cov();
        let sigma = self.sigma_points();

        // Propagate each sigma point through orbital dynamics
        let propagated: Vec<Vector6<f64>> =
            sigma.iter().map(|s| rk4_propagate(s, dt_sec)).collect();

        // Weighted mean
        let mut mean = Vector6::zeros();
        for (i, p) in propagated.iter().enumerate() {
            mean += wm[i] * p;
        }

        // Weighted covariance + process noise
        let mut cov = Matrix6::zeros();
        for (i, p) in propagated.iter().enumerate() {
            let d = p - mean;
            cov += wc[i] * d * d.transpose();
        }
        cov += self.process_noise * dt_sec;

        self.state = mean;
        self.covariance = cov;
    }

    // ── Update step ───────────────────────────────────────────────────────────

    /// Update state using a Doppler observation.
    ///
    /// - `observed_hz`: measured Doppler shift in Hz
    /// - `station_eci`: station ECI position at measurement time (metres)
    /// - `carrier_hz`: RF carrier frequency (Hz)
    pub fn update_with_doppler_measurement(
        &mut self,
        observed_hz: f64,
        station_eci: &Vector3<f64>,
        carrier_hz: f64,
    ) {
        let wm = weights_mean();
        let wc = weights_cov();
        let sigma = self.sigma_points();

        // Predict measurement for each sigma point
        let z_sigma: Vec<f64> = sigma
            .iter()
            .map(|s| doppler_measurement(s, station_eci, carrier_hz))
            .collect();

        // Predicted measurement mean
        let z_mean: f64 = z_sigma.iter().zip(wm.iter()).map(|(z, w)| w * z).sum();

        // Innovation covariance S and cross-covariance Σ_xz
        let mut s_zz = self.measurement_noise;
        let mut sigma_xz = Vector6::zeros();
        for (i, (s, z)) in sigma.iter().zip(z_sigma.iter()).enumerate() {
            let dz = z - z_mean;
            let dx = s - self.state;
            s_zz += wc[i] * dz * dz;
            sigma_xz += wc[i] * dz * dx;
        }

        // Kalman gain
        let kalman_gain = sigma_xz / s_zz;

        // State and covariance update
        let innovation = observed_hz - z_mean;
        self.state += kalman_gain * innovation;
        self.covariance -= s_zz * kalman_gain * kalman_gain.transpose();
    }

    /// Legacy update interface: accepts pre-computed predicted Doppler and
    /// uses a default L-band carrier (1.575 GHz) when carrier is not provided.
    pub fn update_with_doppler(
        &mut self,
        observed_doppler: Frequency,
        _predicted_doppler: Frequency,
        station_pos: Vector3<f64>,
    ) {
        let default_carrier = 1_575_420_000.0_f64; // L1 GPS as fallback
        self.update_with_doppler_measurement(
            observed_doppler as f64,
            &station_pos,
            default_carrier,
        );
    }

    /// Convert back to orbital state
    pub fn to_orbital_state(&self) -> OrbitalState {
        OrbitalState {
            position: Vector3::new(self.state[0], self.state[1], self.state[2]),
            velocity: Vector3::new(self.state[3], self.state[4], self.state[5]),
            time: self.time,
            tle_epoch: self.time,
        }
    }
}

// ── UKF refiner ───────────────────────────────────────────────────────────────

/// UKF refiner — maintains per-satellite filter states and processes
/// Doppler observations to produce refined orbital state estimates.
pub struct UkfRefiner {
    /// UKF state for each tracked satellite
    states: std::collections::HashMap<String, UkfState>,
    /// Station geodetic position (degrees lat/lon, metres alt)
    station_lat: f64,
    station_lon: f64,
    station_alt: f64,
    /// RF carrier frequency for Doppler model (Hz)
    carrier_hz: f64,
}

impl UkfRefiner {
    /// Create a new refiner.
    /// `station_lat/lon` in decimal degrees, `alt_m` in metres above WGS-84 ellipsoid.
    /// `carrier_hz` is the RF carrier (e.g. 437.5e6 for UHF amateur).
    pub fn new(station_lat: f64, station_lon: f64, station_alt: f64) -> Self {
        Self {
            states: std::collections::HashMap::new(),
            station_lat,
            station_lon,
            station_alt,
            carrier_hz: 437_500_000.0, // default: 437.5 MHz UHF
        }
    }

    /// Override the carrier frequency used in the Doppler measurement model.
    pub fn with_carrier_hz(mut self, hz: f64) -> Self {
        self.carrier_hz = hz;
        self
    }

    /// Initialize UKF state for a satellite from an orbital state estimate.
    pub fn initialize(&mut self, satellite_id: String, state: &OrbitalState) {
        self.states
            .insert(satellite_id, UkfState::from_orbital_state(state));
    }

    /// Process a Doppler observation to refine the orbital state.
    pub fn refine_with_observation(
        &mut self,
        satellite_id: &str,
        observed_doppler: Frequency,
        _predicted_doppler: Frequency,
        time: DateTime<Utc>,
    ) -> Result<()> {
        let ukf_state = self.states.get_mut(satellite_id).ok_or_else(|| {
            ground_core::GroundStationError::Tracking(format!(
                "Satellite {} not initialised in UKF",
                satellite_id
            ))
        })?;

        // Predict to observation time using J2-perturbed RK4
        let dt = (time - ukf_state.time).num_milliseconds() as f64 / 1_000.0;
        if dt > 0.0 {
            ukf_state.predict(dt);
            ukf_state.time = time;
        }

        // Compute station ECI at observation time (GMST-rotated)
        let station_eci =
            station_to_eci(self.station_lat, self.station_lon, self.station_alt, time);

        // Update with observed Doppler using full sigma-point measurement model
        ukf_state.update_with_doppler_measurement(
            observed_doppler as f64,
            &station_eci,
            self.carrier_hz,
        );

        Ok(())
    }

    /// Get the current refined orbital state estimate for a satellite.
    pub fn get_refined_state(&self, satellite_id: &str) -> Option<OrbitalState> {
        self.states.get(satellite_id).map(|s| s.to_orbital_state())
    }

    /// Compute the predicted Doppler shift for a satellite at the current time.
    pub fn predict_doppler(&self, satellite_id: &str, time: DateTime<Utc>) -> Option<f64> {
        let ukf_state = self.states.get(satellite_id)?;
        let station_eci =
            station_to_eci(self.station_lat, self.station_lon, self.station_alt, time);
        Some(doppler_measurement(
            &ukf_state.state,
            &station_eci,
            self.carrier_hz,
        ))
    }
}

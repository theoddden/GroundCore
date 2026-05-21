//! Physics systems for digital twin simulation
//!
//! This module implements the Bevy ECS systems that run in parallel to simulate
//! the physics of the constellation. Each system updates a specific aspect of the
//! simulation state.

use crate::components::*;
use bevy_ecs::prelude::*;
use chrono::{DateTime, Duration, Utc};
use ground_core::Result;
use nalgebra::Vector3;
use sgp4::{Constants, MinutesSinceEpoch};
use std::f64::consts::PI;

/// System: Propagate orbits for all satellites
///
/// This system delegates to tracking::Propagator to update satellite positions
/// and velocities based on SGP4 orbital elements. It runs in parallel across all
/// satellite entities.
pub fn propagate_orbits(
    mut query: Query<(&mut OrbitalState, &mut Position, &SatelliteIdComponent)>,
) {
    query.par_iter_mut().for_each(|(mut orbital_state, mut position, satellite_id)| {
        // Compute SGP4 constants if not already cached
        let constants = match Constants::from_elements(&orbital_state.sgp4_elements) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    "Failed to compute SGP4 constants for satellite {}: {}",
                    satellite_id.name,
                    e
                );
                return;
            }
        };

        // Propagate to current time
        let now = Utc::now();
        let tle_epoch = orbital_state.sgp4_elements.datetime.and_utc();
        let minutes_since_epoch = (now - tle_epoch).num_seconds() as f64 / 60.0;

        match constants.propagate(MinutesSinceEpoch(minutes_since_epoch)) {
            Ok(prediction) => {
                position.position = Vector3::new(
                    prediction.position[0],
                    prediction.position[1],
                    prediction.position[2],
                );
                position.velocity = Vector3::new(
                    prediction.velocity[0],
                    prediction.velocity[1],
                    prediction.velocity[2],
                );
                position.time = now;
                orbital_state.mark_updated();
            }
            Err(e) => {
                tracing::error!(
                    "SGP4 propagation failed for satellite {}: {}",
                    satellite_id.name,
                    e
                );
            }
        }
    });
}

/// System: Compute link budget intervals for all active links
///
/// This system computes bounded intervals for link budget parameters (path loss,
/// atmospheric attenuation, predicted SNR) by consuming optical::scintillation
/// for fade distributions. It runs in parallel across all link entities.
pub fn compute_link_intervals(
    mut link_query: Query<(&mut LinkBudget, &OpticalTerminalState)>,
    satellite_query: Query<(&Position, &SatelliteIdComponent)>,
    ground_station_query: Query<(&GroundStation, &Position)>,
) {
    link_query.par_iter_mut().for_each(|(mut link_budget, terminal_state)| {
        // For now, use simplified link budget calculation
        // TODO: Integrate with optical::scintillation for fade distributions

        // Compute path loss based on range (simplified free-space path loss)
        // FSPL(dB) = 20*log10(d) + 20*log10(f) + 92.45
        // where d is distance in km, f is frequency in GHz
        let frequency_ghz = 10.0; // Example: 10 GHz optical link
        let range_km = 1000.0; // Placeholder: should be computed from positions

        let path_loss_db = 20.0 * range_km.log10() + 20.0 * frequency_ghz.log10() + 92.45;

        // Add uncertainty bounds for path loss (±1 dB for range uncertainty)
        let path_loss_interval = crate::divergence::Interval::from_center(path_loss_db, 1.0);

        // Compute atmospheric attenuation using ITU-R P.676
        let atmospheric_db = compute_atmospheric_attenuation(range_km, frequency_ghz);
        let atmospheric_interval = crate::divergence::Interval::from_center(atmospheric_db, 0.5);

        // Compute predicted SNR
        // SNR = TxPower - PathLoss - AtmosphericLoss + RxGain - NoiseFigure
        let tx_power_db = 30.0; // Example: 30 dBm
        let rx_gain_db = 40.0; // Example: 40 dBi
        let noise_figure_db = 3.0; // Example: 3 dB
        let predicted_snr_db = tx_power_db - path_loss_db - atmospheric_db + rx_gain_db - noise_figure_db;

        // Add uncertainty bounds for SNR (±2 dB for combined uncertainties)
        let snr_interval = crate::divergence::Interval::from_center(predicted_snr_db, 2.0);

        link_budget.update(path_loss_interval, atmospheric_interval, snr_interval);
    });
}

/// Compute atmospheric attenuation using ITU-R P.676 gaseous attenuation model
///
/// # Arguments
/// * `range_km` - Slant path range through atmosphere (km)
/// * `frequency_ghz` - Frequency in GHz
///
/// # Returns
/// Atmospheric attenuation in dB
fn compute_atmospheric_attenuation(range_km: f64, frequency_ghz: f64) -> f64 {
    // Simplified ITU-R P.676 implementation
    // This is a placeholder - full implementation would include:
    // - Oxygen absorption lines
    // - Water vapor absorption lines
    // - Pressure, temperature, humidity dependencies

    // For now, use a simple exponential model
    // Attenuation increases with frequency and path length
    let base_attenuation = 0.1 * frequency_ghz; // dB per km at sea level
    let scale_height = 8.0; // km
    let effective_path = range_km * (1.0 - (-range_km / scale_height).exp());

    base_attenuation * effective_path
}

/// System: Compute visibility windows for satellite-ground station pairs
///
/// This system determines which satellites are visible from which ground stations
/// based on elevation angle constraints. It updates the Visibility component for
/// each pair.
pub fn visibility_windows(
    mut visibility_query: Query<(&mut Visibility, &Position)>,
    satellite_query: Query<(&Position, &SatelliteIdComponent)>,
    ground_station_query: Query<(&GroundStation, &Position)>,
) {
    // Minimum elevation angle for visibility (degrees)
    const MIN_ELEVATION_DEG: f64 = 5.0;

    // For each ground station, check visibility to all satellites
    for (ground_station, ground_position) in ground_station_query.iter() {
        for (sat_position, satellite_id) in satellite_query.iter() {
            // Compute range and elevation
            let (range, elevation) = compute_range_and_elevation(
                &sat_position.position,
                &ground_position.position,
                ground_station.lat,
                ground_station.lon,
                sat_position.time,
            );

            // Check if visible
            let is_visible = elevation > MIN_ELEVATION_DEG;

            // Update visibility state (simplified - in practice would need entity relationship)
            // For now, this is a placeholder showing the computation logic
            if is_visible {
                tracing::debug!(
                    "Satellite {} visible from station {} at {:.1}° elevation",
                    satellite_id.name,
                    ground_station.station_id,
                    elevation
                );
            }
        }
    }
}

/// Compute range and elevation angle from ground station to satellite
///
/// # Arguments
/// * `sat_position_eci` - Satellite position in ECI coordinates (km)
/// * `station_position_eci` - Ground station position in ECI coordinates (km)
/// * `station_lat` - Ground station latitude (degrees)
/// * `station_lon` - Ground station longitude (degrees)
/// * `time` - Time of computation
///
/// # Returns
/// (range_km, elevation_deg)
fn compute_range_and_elevation(
    sat_position_eci: &Vector3<f64>,
    station_position_eci: &Vector3<f64>,
    station_lat: f64,
    station_lon: f64,
    time: DateTime<Utc>,
) -> (f64, f64) {
    // Compute relative position vector
    let rel_position = sat_position_eci - station_position_eci;
    let range = rel_position.norm();

    // Convert station position to ECEF (Earth-Centered Earth-Fixed)
    let station_ecef = eci_to_ecef(station_position_eci, time);

    // Convert satellite position to ECEF
    let sat_ecef = eci_to_ecef(sat_position_eci, time);

    // Compute local east, north, up vectors at station
    let (east, north, up) = local_enu_vectors(station_lat, station_lon);

    // Transform satellite position to local ENU coordinates
    let sat_enu = ecef_to_enu(sat_ecef, station_ecef, east, north, up);

    // Compute elevation angle
    let elevation = (sat_enu.z / sat_enu.norm()).asin().to_degrees();

    (range, elevation)
}

/// Convert ECI to ECEF coordinates
fn eci_to_ecef(eci: &Vector3<f64>, time: DateTime<Utc>) -> Vector3<f64> {
    // Compute Earth rotation angle (Greenwich Mean Sidereal Time)
    let theta = gmst(time);

    // Rotation matrix for Earth rotation
    let cos_theta = theta.cos();
    let sin_theta = theta.sin();

    Vector3::new(
        eci.x * cos_theta + eci.y * sin_theta,
        -eci.x * sin_theta + eci.y * cos_theta,
        eci.z,
    )
}

/// Convert ECEF to ENU (East-North-Up) coordinates
fn ecef_to_enu(
    ecef: Vector3<f64>,
    station_ecef: Vector3<f64>,
    east: Vector3<f64>,
    north: Vector3<f64>,
    up: Vector3<f64>,
) -> Vector3<f64> {
    let rel = ecef - station_ecef;
    Vector3::new(
        rel.dot(&east),
        rel.dot(&north),
        rel.dot(&up),
    )
}

/// Compute local ENU (East-North-Up) basis vectors at a given location
fn local_enu_vectors(lat_deg: f64, lon_deg: f64) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>) {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();

    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let sin_lon = lon.sin();
    let cos_lon = lon.cos();

    // East vector
    let east = Vector3::new(-sin_lon, cos_lon, 0.0);

    // North vector
    let north = Vector3::new(-sin_lat * cos_lon, -sin_lat * sin_lon, cos_lat);

    // Up vector
    let up = Vector3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);

    (east, north, up)
}

/// Compute Greenwich Mean Sidereal Time (radians)
fn gmst(time: DateTime<Utc>) -> f64 {
    // Julian date of J2000.0 epoch = 2451545.0
    let jd = 2_451_545.0 + (time.timestamp() as f64 - 946_727_935.816) / 86_400.0;
    let t_ut1 = (jd - 2_451_545.0) / 36_525.0;

    // GMST in seconds (IAU 1982 formula)
    let gmst_sec =
        67_310.548_41 + (8_640_184.812_866 + (0.093_104 - 6.2e-6 * t_ut1) * t_ut1) * t_ut1;

    (gmst_sec * PI / 43_200.0).rem_euclid(2.0 * PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atmospheric_attenuation() {
        let attenuation = compute_atmospheric_attenuation(100.0, 10.0);
        assert!(attenuation > 0.0);
        assert!(attenuation < 100.0); // Reasonable bounds
    }

    #[test]
    fn test_local_enu_vectors() {
        let (east, north, up) = local_enu_vectors(0.0, 0.0);

        // Vectors should be orthogonal
        assert!((east.dot(&north)).abs() < 1e-10);
        assert!((east.dot(&up)).abs() < 1e-10);
        assert!((north.dot(&up)).abs() < 1e-10);

        // Vectors should be normalized
        assert!((east.norm() - 1.0).abs() < 1e-10);
        assert!((north.norm() - 1.0).abs() < 1e-10);
        assert!((up.norm() - 1.0).abs() < 1e-10);
    }
}

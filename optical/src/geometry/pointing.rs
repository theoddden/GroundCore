// Vector computation

use caching::generic::{LruCache, TimeBasedCache};
use chrono::{DateTime, Utc};
use nalgebra::{Unit, Vector3};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Pointing vector
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointingVector {
    pub azimuth_rad: f64,
    pub elevation_rad: f64,
    pub range_km: f64,
}

impl PointingVector {
    pub fn new(azimuth_rad: f64, elevation_rad: f64, range_km: f64) -> Self {
        Self {
            azimuth_rad,
            elevation_rad,
            range_km,
        }
    }

    pub fn to_cartesian(&self) -> Vector3<f64> {
        let x = self.range_km * self.elevation_rad.cos() * self.azimuth_rad.cos();
        let y = self.range_km * self.elevation_rad.cos() * self.azimuth_rad.sin();
        let z = self.range_km * self.elevation_rad.sin();
        Vector3::new(x, y, z)
    }

    pub fn from_cartesian(vec: Vector3<f64>) -> Self {
        let range_km = vec.magnitude();
        let elevation_rad = (vec.z / range_km).asin();
        let azimuth_rad = vec.y.atan2(vec.x);
        Self {
            azimuth_rad,
            elevation_rad,
            range_km,
        }
    }
}

impl Hash for PointingVector {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Convert to integer representation for hashing
        let az = (self.azimuth_rad * 1e9) as i64;
        let el = (self.elevation_rad * 1e9) as i64;
        let rng = (self.range_km * 1e3) as i64;
        az.hash(state);
        el.hash(state);
        rng.hash(state);
    }
}

/// Cache key for pointing vector computations
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PointingCacheKey {
    pub observer_id: String,
    pub target_id: String,
    pub timestamp: DateTime<Utc>,
}

/// Pointing cache for expensive trig computations
pub struct PointingCache {
    cache: LruCache<PointingCacheKey, PointingVector>,
}

impl PointingCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(capacity),
        }
    }

    pub fn get_or_compute<F>(&mut self, key: PointingCacheKey, compute_fn: F) -> PointingVector
    where
        F: FnOnce() -> PointingVector,
    {
        if let Some(cached) = self.cache.get(&key) {
            return cached;
        }
        let computed = compute_fn();
        self.cache.put(key, computed);
        computed
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Satellite position in ECEF coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SatellitePosition {
    pub x_km: f64,
    pub y_km: f64,
    pub z_km: f64,
    pub velocity_x_km_s: f64,
    pub velocity_y_km_s: f64,
    pub velocity_z_km_s: f64,
}

impl SatellitePosition {
    pub fn new(x_km: f64, y_km: f64, z_km: f64) -> Self {
        Self {
            x_km,
            y_km,
            z_km,
            velocity_x_km_s: 0.0,
            velocity_y_km_s: 0.0,
            velocity_z_km_s: 0.0,
        }
    }

    pub fn with_velocity(x_km: f64, y_km: f64, z_km: f64, vx: f64, vy: f64, vz: f64) -> Self {
        Self {
            x_km,
            y_km,
            z_km,
            velocity_x_km_s: vx,
            velocity_y_km_s: vy,
            velocity_z_km_s: vz,
        }
    }

    pub fn to_vector(&self) -> Vector3<f64> {
        Vector3::new(self.x_km, self.y_km, self.z_km)
    }

    pub fn velocity_vector(&self) -> Vector3<f64> {
        Vector3::new(
            self.velocity_x_km_s,
            self.velocity_y_km_s,
            self.velocity_z_km_s,
        )
    }
}

/// Compute pointing vector from observer to target
pub fn compute_pointing_vector(
    observer: &SatellitePosition,
    target: &SatellitePosition,
) -> PointingVector {
    let obs_vec = observer.to_vector();
    let target_vec = target.to_vector();
    let diff = target_vec - obs_vec;
    PointingVector::from_cartesian(diff)
}

/// Compute pointing vector with caching support
pub fn compute_pointing_vector_cached(
    observer: &SatellitePosition,
    target: &SatellitePosition,
    cache: &mut PointingCache,
    observer_id: String,
    target_id: String,
    timestamp: DateTime<Utc>,
) -> PointingVector {
    let key = PointingCacheKey {
        observer_id,
        target_id,
        timestamp,
    };

    cache.get_or_compute(key, || compute_pointing_vector(observer, target))
}

/// Compute relative velocity between two satellites
pub fn compute_relative_velocity(
    observer: &SatellitePosition,
    target: &SatellitePosition,
) -> Vector3<f64> {
    target.velocity_vector() - observer.velocity_vector()
}

/// Compute range rate (rate of change of distance)
pub fn compute_range_rate(observer: &SatellitePosition, target: &SatellitePosition) -> f64 {
    let pointing = compute_pointing_vector(observer, target);
    let rel_vel = compute_relative_velocity(observer, target);
    let unit_pointing = Unit::new_normalize(pointing.to_cartesian());
    rel_vel.dot(&unit_pointing)
}

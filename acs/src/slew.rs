//! Slew rate limiting and motion planning

use crate::controller::{PointingTarget};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Slew identifier
pub type SlewId = Uuid;

/// Motion profile type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionProfile {
    /// Trapezoidal velocity profile
    Trapezoidal,
    /// S-curve velocity profile
    SCurve,
    /// Minimum time (bang-bang)
    MinimumTime,
}

/// Slew trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlewTrajectory {
    /// Slew ID
    pub id: SlewId,
    /// Start position (az, el)
    pub start: (f64, f64),
    /// Target position (az, el)
    pub target: (f64, f64),
    /// Motion profile
    pub profile: MotionProfile,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: DateTime<Utc>,
    /// Azimuth waypoints
    pub azimuth_waypoints: Vec<(DateTime<Utc>, f64)>,
    /// Elevation waypoints
    pub elevation_waypoints: Vec<(DateTime<Utc>, f64)>,
}

/// Slew limiter
pub struct SlewLimiter {
    /// Maximum azimuth slew rate (degrees per second)
    azimuth_rate_limit: f64,
    /// Maximum elevation slew rate (degrees per second)
    elevation_rate_limit: f64,
    /// Maximum azimuth acceleration (degrees per second squared)
    azimuth_accel_limit: f64,
    /// Maximum elevation acceleration (degrees per second squared)
    elevation_accel_limit: f64,
    /// Default motion profile
    default_profile: MotionProfile,
}

impl SlewLimiter {
    /// Create a new slew limiter
    pub fn new(
        azimuth_rate_limit: f64,
        elevation_rate_limit: f64,
        azimuth_accel_limit: f64,
        elevation_accel_limit: f64,
        default_profile: MotionProfile,
    ) -> Self {
        Self {
            azimuth_rate_limit,
            elevation_rate_limit,
            azimuth_accel_limit,
            elevation_accel_limit,
            default_profile,
        }
    }

    /// Compute slew trajectory respecting rate and acceleration limits
    pub fn compute_trajectory(
        &self,
        start: (f64, f64),
        target: (f64, f64),
        start_time: DateTime<Utc>,
    ) -> Result<SlewTrajectory, String> {
        let id = Uuid::new_v4();
        
        // Compute minimum time trajectory
        let (az_duration, el_duration) = self.compute_minimum_time(start, target)?;
        
        // Overall duration is the max of the two axes
        let duration = az_duration.max(el_duration);
        let end_time = start_time + Duration::milliseconds((duration * 1000.0) as i64);
        
        // Generate waypoints based on motion profile
        let (az_waypoints, el_waypoints) = self.generate_waypoints(
            start,
            target,
            start_time,
            duration,
            self.default_profile,
        )?;
        
        Ok(SlewTrajectory {
            id,
            start,
            target,
            profile: self.default_profile,
            start_time,
            end_time,
            azimuth_waypoints,
            elevation_waypoints,
        })
    }

    /// Compute minimum time for slew respecting rate and acceleration limits
    fn compute_minimum_time(&self, start: (f64, f64), target: (f64, f64)) -> Result<(f64, f64), String> {
        let az_distance = (target.0 - start.0).abs();
        let el_distance = (target.1 - start.1).abs();
        
        // Minimum time = time to accelerate + time at max rate + time to decelerate
        // t_min = 2 * sqrt(distance / acceleration) if distance < rate^2 / acceleration
        // Otherwise: t_min = distance / rate + rate / acceleration
        
        let az_min_time = self.axis_minimum_time(az_distance, self.azimuth_rate_limit, self.azimuth_accel_limit);
        let el_min_time = self.axis_minimum_time(el_distance, self.elevation_rate_limit, self.elevation_accel_limit);
        
        Ok((az_min_time, el_min_time))
    }

    /// Compute minimum time for single axis
    fn axis_minimum_time(&self, distance: f64, rate_limit: f64, accel_limit: f64) -> f64 {
        if distance < 1e-6 {
            return 0.0;
        }
        
        let critical_distance = rate_limit * rate_limit / accel_limit;
        
        if distance < critical_distance {
            // Triangle profile (accelerate then decelerate, never reach max rate)
            2.0 * (distance / accel_limit).sqrt()
        } else {
            // Trapezoidal profile (accelerate, coast at max rate, decelerate)
            distance / rate_limit + rate_limit / accel_limit
        }
    }

    /// Generate waypoints for trajectory
    fn generate_waypoints(
        &self,
        start: (f64, f64),
        target: (f64, f64),
        start_time: DateTime<Utc>,
        duration: f64,
        profile: MotionProfile,
    ) -> Result<(Vec<(DateTime<Utc>, f64)>, Vec<(DateTime<Utc>, f64)>), String> {
        let num_waypoints = 20;
        let dt = duration / (num_waypoints as f64);
        
        let mut az_waypoints = Vec::new();
        let mut el_waypoints = Vec::new();
        
        for i in 0..=num_waypoints {
            let t = (i as f64) * dt;
            let time = start_time + Duration::milliseconds((t * 1000.0) as i64);
            
            let progress = match profile {
                MotionProfile::Trapezoidal => self.trapezoidal_progress(t, duration),
                MotionProfile::SCurve => self.scurve_progress(t, duration),
                MotionProfile::MinimumTime => self.trapezoidal_progress(t, duration),
            };
            
            let az = start.0 + (target.0 - start.0) * progress;
            let el = start.1 + (target.1 - start.1) * progress;
            
            az_waypoints.push((time, az));
            el_waypoints.push((time, el));
        }
        
        Ok((az_waypoints, el_waypoints))
    }

    /// Trapezoidal velocity profile progress
    fn trapezoidal_progress(&self, t: f64, duration: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= duration {
            return 1.0;
        }
        
        // Simplified trapezoidal (linear acceleration and deceleration)
        let accel_time = duration * 0.2;
        let coast_time = duration * 0.6;
        let decel_time = duration * 0.2;
        
        if t < accel_time {
            // Acceleration phase
            0.5 * (t / accel_time).powi(2)
        } else if t < accel_time + coast_time {
            // Coast phase
            0.5 + (t - accel_time) / coast_time * 0.5
        } else {
            // Deceleration phase
            let t_decel = t - (accel_time + coast_time);
            1.0 - 0.5 * ((decel_time - t_decel) / decel_time).powi(2)
        }
    }

    /// S-curve velocity profile progress
    fn scurve_progress(&self, t: f64, duration: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= duration {
            return 1.0;
        }
        
        // S-curve using sine function for smooth acceleration
        let normalized_t = t / duration;
        0.5 * (1.0 - (normalized_t * std::f64::consts::PI).cos())
    }

    /// Check if slew is possible within time limit
    pub fn can_slew_in_time(
        &self,
        start: (f64, f64),
        target: (f64, f64),
        time_limit: f64,
    ) -> bool {
        let (az_min_time, el_min_time) = self.compute_minimum_time(start, target).unwrap_or((f64::MAX, f64::MAX));
        az_min_time.max(el_min_time) <= time_limit
    }

    /// Get rate limits
    pub fn get_rate_limits(&self) -> (f64, f64) {
        (self.azimuth_rate_limit, self.elevation_rate_limit)
    }

    /// Get acceleration limits
    pub fn get_accel_limits(&self) -> (f64, f64) {
        (self.azimuth_accel_limit, self.elevation_accel_limit)
    }
}

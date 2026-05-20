//! Safety interlocks and limits

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Interlock state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterlockState {
    /// Interlock not triggered
    Normal,
    /// Position limit exceeded
    PositionLimit,
    /// Collision detected
    Collision,
    /// Motor fault
    MotorFault,
    /// Emergency stop activated
    EmergencyStop,
    /// Watchdog timeout
    WatchdogTimeout,
    /// Communication lost
    CommunicationLost,
}

/// Safety interlock
pub struct SafetyInterlock {
    /// Current interlock state
    state: RwLock<InterlockState>,
    /// Azimuth minimum limit (degrees)
    az_min: f64,
    /// Azimuth maximum limit (degrees)
    az_max: f64,
    /// Elevation minimum limit (degrees)
    el_min: f64,
    /// Elevation maximum limit (degrees)
    el_max: f64,
    /// Emergency stop flag
    emergency_stop: AtomicBool,
    /// Watchdog last heartbeat timestamp
    last_heartbeat: AtomicU64,
    /// Watchdog timeout (milliseconds)
    watchdog_timeout_ms: u64,
    /// Collision zones (az_min, az_max, el_min, el_max)
    collision_zones: Vec<(f64, f64, f64, f64)>,
}

impl SafetyInterlock {
    /// Create a new safety interlock
    pub fn new(
        az_min: f64,
        az_max: f64,
        el_min: f64,
        el_max: f64,
        watchdog_timeout_ms: u64,
    ) -> Self {
        Self {
            state: RwLock::new(InterlockState::Normal),
            az_min,
            az_max,
            el_min,
            el_max,
            emergency_stop: AtomicBool::new(false),
            last_heartbeat: AtomicU64::new(Utc::now().timestamp_millis() as u64),
            watchdog_timeout_ms,
            collision_zones: Vec::new(),
        }
    }

    /// Check if position is within limits
    pub async fn check_position_limits(&self, azimuth: f64, elevation: f64) -> bool {
        azimuth >= self.az_min
            && azimuth <= self.az_max
            && elevation >= self.el_min
            && elevation <= self.el_max
    }

    /// Check for collision with defined zones
    pub async fn check_collision(&self, azimuth: f64, elevation: f64) -> bool {
        for (az_min, az_max, el_min, el_max) in &self.collision_zones {
            if azimuth >= *az_min
                && azimuth <= *az_max
                && elevation >= *el_min
                && elevation <= *el_max
            {
                return true;
            }
        }
        false
    }

    /// Add a collision zone to avoid
    pub async fn add_collision_zone(&mut self, az_min: f64, az_max: f64, el_min: f64, el_max: f64) {
        self.collision_zones.push((az_min, az_max, el_min, el_max));
    }

    /// Trigger emergency stop
    pub async fn trigger_emergency_stop(&self) {
        self.emergency_stop.store(true, Ordering::Relaxed);
        let mut state = self.state.write().await;
        *state = InterlockState::EmergencyStop;
    }

    /// Clear emergency stop
    pub async fn clear_emergency_stop(&self) {
        self.emergency_stop.store(false, Ordering::Relaxed);
        let mut state = self.state.write().await;
        *state = InterlockState::Normal;
    }

    /// Check if emergency stop is active
    pub fn is_emergency_stop(&self) -> bool {
        self.emergency_stop.load(Ordering::Relaxed)
    }

    /// Update watchdog heartbeat
    pub fn update_watchdog(&self) {
        self.last_heartbeat
            .store(Utc::now().timestamp_millis() as u64, Ordering::Relaxed);
    }

    /// Check watchdog timeout
    pub fn check_watchdog(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::Relaxed) as i64;
        let now = Utc::now().timestamp_millis();
        (now - last) < self.watchdog_timeout_ms as i64
    }

    /// Validate position and update interlock state
    pub async fn validate_position(
        &self,
        azimuth: f64,
        elevation: f64,
    ) -> Result<(), InterlockState> {
        if self.is_emergency_stop() {
            let mut state = self.state.write().await;
            *state = InterlockState::EmergencyStop;
            return Err(InterlockState::EmergencyStop);
        }

        if !self.check_position_limits(azimuth, elevation).await {
            let mut state = self.state.write().await;
            *state = InterlockState::PositionLimit;
            return Err(InterlockState::PositionLimit);
        }

        if self.check_collision(azimuth, elevation).await {
            let mut state = self.state.write().await;
            *state = InterlockState::Collision;
            return Err(InterlockState::Collision);
        }

        if !self.check_watchdog() {
            let mut state = self.state.write().await;
            *state = InterlockState::WatchdogTimeout;
            return Err(InterlockState::WatchdogTimeout);
        }

        let mut state = self.state.write().await;
        *state = InterlockState::Normal;
        Ok(())
    }

    /// Get current interlock state
    pub async fn get_state(&self) -> InterlockState {
        let state = self.state.read().await;
        *state
    }

    /// Set motor fault
    pub async fn set_motor_fault(&self) {
        let mut state = self.state.write().await;
        *state = InterlockState::MotorFault;
    }

    /// Clear motor fault
    pub async fn clear_motor_fault(&self) {
        let mut state = self.state.write().await;
        if *state == InterlockState::MotorFault {
            *state = InterlockState::Normal;
        }
    }

    /// Set communication lost
    pub async fn set_communication_lost(&self) {
        let mut state = self.state.write().await;
        *state = InterlockState::CommunicationLost;
    }

    /// Clear communication lost
    pub async fn clear_communication_lost(&self) {
        let mut state = self.state.write().await;
        if *state == InterlockState::CommunicationLost {
            *state = InterlockState::Normal;
        }
    }
}

/// Emergency stop
pub struct EmergencyStop {
    triggered: AtomicBool,
    trigger_time: RwLock<Option<DateTime<Utc>>>,
    trigger_reason: RwLock<Option<String>>,
}

impl Default for EmergencyStop {
    fn default() -> Self {
        Self::new()
    }
}

impl EmergencyStop {
    /// Create a new emergency stop
    pub fn new() -> Self {
        Self {
            triggered: AtomicBool::new(false),
            trigger_time: RwLock::new(None),
            trigger_reason: RwLock::new(None),
        }
    }

    /// Trigger emergency stop
    pub async fn trigger(&self, reason: &str) {
        self.triggered.store(true, Ordering::Relaxed);
        let mut time = self.trigger_time.write().await;
        *time = Some(Utc::now());
        let mut r = self.trigger_reason.write().await;
        *r = Some(reason.to_string());
    }

    /// Clear emergency stop
    pub async fn clear(&self) {
        self.triggered.store(false, Ordering::Relaxed);
        let mut time = self.trigger_time.write().await;
        *time = None;
        let mut r = self.trigger_reason.write().await;
        *r = None;
    }

    /// Check if emergency stop is triggered
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::Relaxed)
    }

    /// Get trigger time
    pub async fn get_trigger_time(&self) -> Option<DateTime<Utc>> {
        let time = self.trigger_time.read().await;
        *time
    }

    /// Get trigger reason
    pub async fn get_trigger_reason(&self) -> Option<String> {
        let r = self.trigger_reason.read().await;
        r.clone()
    }
}

//! Failure injection for testing failover scenarios

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Types of failures to inject
#[derive(Debug, Clone, Copy)]
pub enum FailureType {
    /// SDR stops responding
    SdrUnresponsive,
    /// SDR returns garbage data
    SdrGarbageData,
    /// SDR frequency drift
    FrequencyDrift { drift_hz_per_sec: f64 },
    /// SDR sample rate deviation
    SampleRateDeviation { deviation_ppm: f64 },
    /// Intermittent packet loss
    PacketLoss { loss_rate: f64 },
    /// SDR overheats
    Overheat,
}

/// Failure injector for testing
#[derive(Clone)]
pub struct FailureInjector {
    enabled: bool,
    failure_type: Option<FailureType>,
    failure_time: Option<DateTime<Utc>>,
    recovery_time: Option<DateTime<Utc>>,
    current_state: FailureState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureState {
    Normal,
    #[allow(dead_code)]
    Failing,
    #[allow(dead_code)]
    Recovered,
}

impl Default for FailureInjector {
    fn default() -> Self {
        Self {
            enabled: false,
            failure_type: None,
            failure_time: None,
            recovery_time: None,
            current_state: FailureState::Normal,
        }
    }
}

impl FailureInjector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule a failure
    pub fn schedule_failure(&mut self, failure_type: FailureType, delay: Duration) {
        self.enabled = true;
        self.failure_type = Some(failure_type);
        self.failure_time = Some(Utc::now() + chrono::Duration::from_std(delay).unwrap());
        self.recovery_time = None;
        self.current_state = FailureState::Normal;
    }

    /// Schedule a failure with automatic recovery
    pub fn schedule_failure_with_recovery(
        &mut self,
        failure_type: FailureType,
        delay: Duration,
        recovery_after: Duration,
    ) {
        self.enabled = true;
        self.failure_type = Some(failure_type);
        self.failure_time = Some(Utc::now() + chrono::Duration::from_std(delay).unwrap());
        self.recovery_time =
            Some(Utc::now() + chrono::Duration::from_std(delay + recovery_after).unwrap());
        self.current_state = FailureState::Normal;
    }

    /// Check if a failure is currently active
    pub fn is_failing(&self) -> bool {
        if !self.enabled {
            return false;
        }

        let now = Utc::now();

        match (self.failure_time, self.recovery_time) {
            (Some(failure), None) => now >= failure,
            (Some(failure), Some(recovery)) => now >= failure && now < recovery,
            _ => false,
        }
    }

    /// Check if recovery has occurred
    pub fn has_recovered(&self) -> bool {
        if !self.enabled {
            return false;
        }

        match self.recovery_time {
            Some(recovery) => Utc::now() >= recovery,
            None => false,
        }
    }

    /// Apply failure effect to sample data
    pub fn apply_failure(&mut self, i: &mut f32, q: &mut f32) -> bool {
        if !self.is_failing() {
            return false;
        }

        if self.failure_time.is_none() {
            self.failure_time = Some(Utc::now());
        }

        if let Some(failure_type) = self.failure_type {
            match failure_type {
                FailureType::SdrUnresponsive => {
                    // Return true to indicate SDR is unresponsive
                    return true;
                }
                FailureType::SdrGarbageData => {
                    *i = rand::random::<f32>();
                    *q = rand::random::<f32>();
                }
                FailureType::FrequencyDrift { drift_hz_per_sec } => {
                    let elapsed = (Utc::now() - self.failure_time.unwrap()).num_seconds() as f64;
                    let phase_drift = drift_hz_per_sec * elapsed;
                    let cos = phase_drift.cos();
                    let sin = phase_drift.sin();
                    let new_i = *i * cos as f32 - *q * sin as f32;
                    let new_q = *i * sin as f32 + *q * cos as f32;
                    *i = new_i;
                    *q = new_q;
                }
                FailureType::SampleRateDeviation { deviation_ppm: _ } => {
                    // Sample rate deviation is handled at the sample generation level
                    // This is a placeholder for that effect
                }
                FailureType::PacketLoss { loss_rate } => {
                    if rand::random::<f64>() < loss_rate {
                        // Drop this sample
                        return true;
                    }
                }
                FailureType::Overheat => {
                    // Reduce signal amplitude
                    *i *= 0.1;
                    *q *= 0.1;
                }
            }
        }

        false
    }

    /// Reset the failure injector
    pub fn reset(&mut self) {
        self.enabled = false;
        self.failure_type = None;
        self.failure_time = None;
        self.recovery_time = None;
        self.current_state = FailureState::Normal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_failure_scheduling() {
        let mut injector = FailureInjector::new();

        injector.schedule_failure(FailureType::SdrGarbageData, Duration::from_millis(100));

        assert!(!injector.is_failing());

        thread::sleep(Duration::from_millis(150));

        assert!(injector.is_failing());
    }

    #[test]
    fn test_failure_with_recovery() {
        let mut injector = FailureInjector::new();

        injector.schedule_failure_with_recovery(
            FailureType::SdrGarbageData,
            Duration::from_millis(100),
            Duration::from_millis(200),
        );

        thread::sleep(Duration::from_millis(150));
        assert!(injector.is_failing());

        thread::sleep(Duration::from_millis(250));
        assert!(injector.has_recovered());
        assert!(!injector.is_failing());
    }
}

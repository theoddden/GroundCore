//! Simulated SDR implementation

use crate::failure::FailureInjector;
use crate::signal::{NoiseModel, SignalGenerator, SignalType};
use chrono::{DateTime, Utc};
use ground_core::{Frequency, Result};
use rf_layer::sdr::{Sample, SampleId};
use std::sync::atomic::{AtomicU64, Ordering};

/// Configuration for simulated SDR
#[derive(Debug, Clone)]
pub struct SimulatedSdrConfig {
    /// Device ID
    pub device_id: String,
    /// Carrier frequency
    pub carrier_frequency: Frequency,
    /// Sample rate
    pub sample_rate: u64,
    /// Signal type
    pub signal_type: SignalType,
    /// Noise model
    pub noise_model: NoiseModel,
    /// Simulated Doppler shift (Hz)
    pub doppler_shift: i64,
    /// Doppler drift rate (Hz/sec)
    pub doppler_drift_rate: f64,
}

impl Default for SimulatedSdrConfig {
    fn default() -> Self {
        Self {
            device_id: "sim-sdr-0".to_string(),
            carrier_frequency: 1_600_000_000, // L-band
            sample_rate: 2_000_000,           // 2 MHz
            signal_type: SignalType::SineWave,
            noise_model: NoiseModel::Awgn { snr_db: 20.0 },
            doppler_shift: 0,
            doppler_drift_rate: 0.0,
        }
    }
}

/// Simulated SDR device
pub struct SimulatedSdr {
    config: SimulatedSdrConfig,
    signal_generator: SignalGenerator,
    failure_injector: FailureInjector,
    sample_counter: AtomicU64,
    _current_doppler: f64,
    start_time: DateTime<Utc>,
    enabled: bool,
}

impl SimulatedSdr {
    pub fn new(config: SimulatedSdrConfig) -> Self {
        let doppler_shift = config.doppler_shift;
        let signal_generator = SignalGenerator::new(
            config.signal_type,
            config.noise_model,
            config.carrier_frequency,
            config.sample_rate,
        );

        Self {
            config,
            signal_generator,
            failure_injector: FailureInjector::new(),
            sample_counter: AtomicU64::new(0),
            _current_doppler: doppler_shift as f64,
            start_time: Utc::now(),
            enabled: true,
        }
    }

    /// Get device ID
    pub fn device_id(&self) -> &str {
        &self.config.device_id
    }

    /// Get the failure injector
    pub fn failure_injector(&mut self) -> &mut FailureInjector {
        &mut self.failure_injector
    }

    /// Tune to a specific frequency
    pub fn tune(&mut self, frequency: Frequency) -> Result<()> {
        self.config.carrier_frequency = frequency;
        self.signal_generator = SignalGenerator::new(
            self.config.signal_type,
            self.config.noise_model,
            frequency,
            self.config.sample_rate,
        );
        Ok(())
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, rate: u64) -> Result<()> {
        self.config.sample_rate = rate;
        self.signal_generator = SignalGenerator::new(
            self.config.signal_type,
            self.config.noise_model,
            self.config.carrier_frequency,
            rate,
        );
        Ok(())
    }

    /// Enable or disable the SDR
    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        self.enabled = enabled;
        Ok(())
    }

    /// Read samples from the simulated SDR
    pub fn read_samples(&mut self, buffer: &mut [Sample]) -> Result<usize> {
        if !self.enabled {
            return Ok(0);
        }

        // Update Doppler shift over time
        let elapsed = (Utc::now() - self.start_time).num_seconds() as f64;
        let current_doppler =
            self.config.doppler_shift as f64 + self.config.doppler_drift_rate * elapsed;

        let count = buffer.len();
        for (_i, sample) in buffer.iter_mut().enumerate() {
            let sample_id = self.sample_counter.fetch_add(1, Ordering::SeqCst);
            let now = Utc::now();

            // Generate signal
            let (i, q) = self.signal_generator.next_sample();

            // Apply Doppler shift (simplified as frequency offset)
            let doppler_phase = 2.0 * std::f64::consts::PI * current_doppler * i as f64
                / self.config.sample_rate as f64;
            let cos = doppler_phase.cos();
            let sin = doppler_phase.sin();
            let mut doppler_i = i * cos as f32 - q * sin as f32;
            let mut doppler_q = i * sin as f32 + q * cos as f32;

            // Apply failure injection
            let is_failed = self
                .failure_injector
                .apply_failure(&mut doppler_i, &mut doppler_q);

            if is_failed {
                // SDR is unresponsive, return count of samples written before failure
                return Ok(_i);
            }

            *sample = Sample {
                i: doppler_i,
                q: doppler_q,
                event_time: now, // Would be computed from orbital position
                reception_time: now,
                id: SampleId::new(sample_id),
            };
        }

        Ok(count)
    }

    /// Get current configuration
    pub fn config(&self) -> &SimulatedSdrConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: SimulatedSdrConfig) {
        self.config = config;
        self.signal_generator = SignalGenerator::new(
            self.config.signal_type,
            self.config.noise_model,
            self.config.carrier_frequency,
            self.config.sample_rate,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure::FailureType;

    #[tokio::test]
    async fn test_simulated_sdr_basic() {
        let config = SimulatedSdrConfig::default();
        let mut sdr = SimulatedSdr::new(config);

        let mut buffer = vec![
            Sample {
                i: 0.0,
                q: 0.0,
                event_time: Utc::now(),
                reception_time: Utc::now(),
                id: SampleId::new(0),
            };
            100
        ];

        let count = sdr.read_samples(&mut buffer).unwrap();
        assert_eq!(count, 100);

        assert!(buffer[0].i != 0.0 || buffer[0].q != 0.0);
    }

    #[tokio::test]
    async fn test_doppler_simulation() {
        let config = SimulatedSdrConfig {
            doppler_shift: 1000,
            doppler_drift_rate: 100.0,
            ..Default::default()
        };

        let mut sdr = SimulatedSdr::new(config);

        // Use tokio::time::sleep to avoid blocking the executor thread
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut buffer = vec![
            Sample {
                i: 0.0,
                q: 0.0,
                event_time: Utc::now(),
                reception_time: Utc::now(),
                id: SampleId::new(0),
            };
            10
        ];

        sdr.read_samples(&mut buffer).unwrap();
        // Doppler should have drifted from the initial shift
    }

    #[tokio::test]
    async fn test_failure_injection() {
        let config = SimulatedSdrConfig::default();
        let mut sdr = SimulatedSdr::new(config);

        sdr.failure_injector().schedule_failure(
            FailureType::SdrGarbageData,
            std::time::Duration::from_millis(50),
        );

        // Use tokio::time::sleep to avoid blocking the executor thread
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut buffer = vec![
            Sample {
                i: 0.0,
                q: 0.0,
                event_time: Utc::now(),
                reception_time: Utc::now(),
                id: SampleId::new(0),
            };
            10
        ];

        sdr.read_samples(&mut buffer).unwrap();
        // Samples should be garbage now
    }
}

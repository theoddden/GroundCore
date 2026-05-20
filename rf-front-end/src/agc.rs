//! AGC controller with automatic gain adjustment

use crate::amplifier::{AmplifierControl, AmplifierStatus};
use crate::attenuator::AttenuatorControl;
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

/// AGC mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgcMode {
    /// Manual gain control
    Manual,
    /// Automatic gain control
    Automatic,
}

/// AGC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgcConfig {
    /// Target signal level (dBFS)
    pub target_level: f64,
    /// Attack time (milliseconds)
    pub attack_time: u64,
    /// Release time (milliseconds)
    pub release_time: u64,
    /// Maximum gain (dB)
    pub max_gain: f64,
    /// Minimum gain (dB)
    pub min_gain: f64,
    /// Hysteresis (dB)
    pub hysteresis: f64,
}

impl Default for AgcConfig {
    fn default() -> Self {
        Self {
            target_level: -10.0,
            attack_time: 10,
            release_time: 100,
            max_gain: 60.0,
            min_gain: 0.0,
            hysteresis: 2.0,
        }
    }
}

/// AGC controller
pub struct AgcController {
    config: AgcConfig,
    mode: RwLock<AgcMode>,
    current_level: RwLock<f64>,
    amplifiers: RwLock<Vec<Box<dyn AmplifierControl + Send + Sync>>>,
    attenuators: RwLock<Vec<Box<dyn AttenuatorControl + Send + Sync>>>,
    enabled: AtomicBool,
}

impl AgcController {
    /// Create a new AGC controller
    pub fn new(config: AgcConfig) -> Self {
        Self {
            config,
            mode: RwLock::new(AgcMode::Manual),
            current_level: RwLock::new(-20.0),
            amplifiers: RwLock::new(Vec::new()),
            attenuators: RwLock::new(Vec::new()),
            enabled: AtomicBool::new(false),
        }
    }

    /// Add an amplifier to the AGC chain
    pub async fn add_amplifier(&self, amplifier: Box<dyn AmplifierControl + Send + Sync>) {
        let mut amplifiers = self.amplifiers.write().await;
        amplifiers.push(amplifier);
    }

    /// Add an attenuator to the AGC chain
    pub async fn add_attenuator(&self, attenuator: Box<dyn AttenuatorControl + Send + Sync>) {
        let mut attenuators = self.attenuators.write().await;
        attenuators.push(attenuator);
    }

    /// Set AGC mode
    pub async fn set_mode(&self, mode: AgcMode) {
        let mut current_mode = self.mode.write().await;
        *current_mode = mode;
    }

    /// Get AGC mode
    pub async fn get_mode(&self) -> AgcMode {
        let mode = self.mode.read().await;
        *mode
    }

    /// Enable AGC
    pub async fn enable(&self) {
        self.enabled.store(1.0, Ordering::Relaxed);
    }

    /// Disable AGC
    pub async fn disable(&self) {
        self.enabled.store(0.0, Ordering::Relaxed);
    }

    /// Check if AGC is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed) > 0.0
    }

    /// Update signal level
    pub async fn update_signal_level(&self, level: f64) {
        *self.current_level.write().await = level;

        if self.is_enabled() && self.get_mode().await == AgcMode::Automatic {
            if let Err(e) = self.adjust_gain().await {
                tracing::warn!("AGC adjustment failed: {}", e);
            }
        }
    }

    /// Get current signal level
    pub async fn get_current_level(&self) -> f64 {
        *self.current_level.read().await
    }

    /// Adjust gain based on current signal level
    async fn adjust_gain(&self) -> Result<()> {
        let current_level = *self.current_level.read().await;
        let target_level = self.config.target_level;
        let error = target_level - current_level;

        // Apply hysteresis
        if error.abs() < self.config.hysteresis {
            return Ok(());
        }

        // Compute required adjustment
        let adjustment = self.compute_adjustment(error);

        // Apply adjustment to amplifiers
        let amplifiers = self.amplifiers.read().await;
        for amp in amplifiers.iter() {
            if let Ok(current_gain) = amp.get_gain().await {
                let new_gain =
                    (current_gain + adjustment).clamp(self.config.min_gain, self.config.max_gain);
                if let Err(e) = amp.set_gain(new_gain).await {
                    tracing::warn!("Failed to set amplifier gain: {}", e);
                }
            }
        }

        // Apply adjustment to attenuators (inverse relationship)
        let attenuators = self.attenuators.read().await;
        for att in attenuators.iter() {
            if let Ok(current_atten) = att.get_attenuation().await {
                let new_atten = (current_atten - adjustment).clamp(0.0, 31.5);
                if let Err(e) = att.set_attenuation(new_atten).await {
                    tracing::warn!("Failed to set attenuator: {}", e);
                }
            }
        }

        tracing::debug!(
            "AGC adjusted gain by {} dB (error: {} dB)",
            adjustment,
            error
        );
        Ok(())
    }

    /// Compute gain adjustment based on error
    fn compute_adjustment(&self, error: f64) -> f64 {
        // Simple proportional control
        let kp = 0.5;
        kp * error
    }

    /// Manually set gain (for manual mode)
    pub async fn set_manual_gain(&self, gain_db: f64) -> Result<()> {
        let amplifiers = self.amplifiers.read().await;

        for amp in amplifiers.iter() {
            if let Err(e) = amp.set_gain(gain_db).await {
                tracing::warn!("Failed to set amplifier gain: {}", e);
            }
        }

        // Reset attenuators to minimum
        let attenuators = self.attenuators.read().await;
        for att in attenuators.iter() {
            if let Err(e) = att.set_attenuation(0.0).await {
                tracing::warn!("Failed to reset attenuator: {}", e);
            }
        }

        Ok(())
    }

    /// Get total gain
    pub async fn get_total_gain(&self) -> Result<f64> {
        let mut total_gain = 0.0;

        let amplifiers = self.amplifiers.read().await;
        for amp in amplifiers.iter() {
            if let Ok(gain) = amp.get_gain().await {
                total_gain += gain;
            }
        }

        let attenuators = self.attenuators.read().await;
        for att in attenuators.iter() {
            if let Ok(atten) = att.get_attenuation().await {
                total_gain -= atten;
            }
        }

        Ok(total_gain)
    }

    /// Get AGC status
    pub async fn get_status(&self) -> AgcStatus {
        let mode = self.mode.read().await;
        let current_level = self.current_level.load(Ordering::Relaxed);
        let total_gain = self.get_total_gain().await.unwrap_or(0.0);

        AgcStatus {
            enabled: self.is_enabled(),
            mode: *mode,
            current_level,
            target_level: self.config.target_level,
            total_gain,
            error: self.config.target_level - current_level,
        }
    }
}

/// AGC status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgcStatus {
    pub enabled: bool,
    pub mode: AgcMode,
    pub current_level: f64,
    pub target_level: f64,
    pub total_gain: f64,
    pub error: f64,
}

//! Amplifier control with gain staging and thermal protection

use crate::device::{RfDevice, RfDeviceType, RfDeviceState, DeviceCalibration};
use ground_core::{Result, GroundStationError};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicF64, AtomicU64, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Gain stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GainStage {
    /// Stage index
    pub index: usize,
    /// Gain in dB
    pub gain_db: f64,
    /// Noise figure in dB
    pub_noise_figure: f64,
    /// Maximum input power (dBm)
    pub max_input_power: f64,
    /// Enabled flag
    pub enabled: bool,
}

/// Amplifier status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmplifierStatus {
    Idle,
    Active,
    Overheating,
    Fault,
    Disabled,
}

/// Amplifier control trait
#[async_trait::async_trait]
pub trait AmplifierControl: RfDevice {
    /// Set gain in dB
    async fn set_gain(&mut self, gain_db: f64) -> Result<()>;
    
    /// Get current gain in dB
    async fn get_gain(&self) -> Result<f64>;
    
    /// Enable amplifier
    async fn enable(&mut self) -> Result<()>;
    
    /// Disable amplifier
    async fn disable(&mut self) -> Result<()>;
    
    /// Get temperature in Celsius
    async fn get_temperature(&self) -> Result<f64>;
    
    /// Get noise figure in dB
    async fn get_noise_figure(&self) -> Result<f64>;
    
    /// Get amplifier status
    async fn get_amplifier_status(&self) -> Result<AmplifierStatus>;
    
    /// Configure gain stages
    async fn configure_gain_stages(&mut self, stages: Vec<GainStage>) -> Result<()>;
    
    /// Get gain stages
    async fn get_gain_stages(&self) -> Result<Vec<GainStage>>;
}

/// Generic amplifier implementation
pub struct GenericAmplifier {
    id: Uuid,
    device_type: RfDeviceType,
    current_gain: AtomicF64,
    temperature: AtomicF64,
    max_temperature: f64,
    thermal_protection_enabled: AtomicBool,
    enabled: AtomicBool,
    fault: AtomicBool,
    gain_stages: RwLock<Vec<GainStage>>,
    amplifier_status: RwLock<AmplifierStatus>,
}

impl GenericAmplifier {
    /// Create a new generic amplifier
    pub fn new(device_type: RfDeviceType, max_temperature: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            device_type,
            current_gain: AtomicF64::new(0.0),
            temperature: AtomicF64::new(25.0),
            max_temperature,
            thermal_protection_enabled: AtomicBool::new(true),
            enabled: AtomicBool::new(false),
            fault: AtomicBool::new(false),
            gain_stages: RwLock::new(Vec::new()),
            amplifier_status: RwLock::new(AmplifierStatus::Idle),
        }
    }

    /// Check thermal protection
    async fn check_thermal_protection(&self) {
        if !self.thermal_protection_enabled.load(Ordering::Relaxed) {
            return;
        }

        let temp = self.temperature.load(Ordering::Relaxed);
        if temp >= self.max_temperature {
            // Trigger thermal protection
            self.fault.store(true, Ordering::Relaxed);
            let mut status = self.amplifier_status.write().await;
            *status = AmplifierStatus::Overheating;
        }
    }

    /// Update temperature (simulated)
    pub async fn update_temperature(&self, temp: f64) {
        self.temperature.store(temp, Ordering::Relaxed);
        self.check_thermal_protection().await;
    }
}

#[async_trait::async_trait]
impl RfDevice for GenericAmplifier {
    fn get_type(&self) -> RfDeviceType {
        self.device_type
    }

    async fn get_state(&self) -> Result<RfDeviceState> {
        let status = self.amplifier_status.read().await;
        Ok(RfDeviceState {
            device_id: self.id,
            device_type: self.device_type,
            enabled: self.enabled.load(Ordering::Relaxed),
            settings: serde_json::json!({
                "gain_db": self.current_gain.load(Ordering::Relaxed),
                "temperature": self.temperature.load(Ordering::Relaxed),
                "status": status,
            }),
            temperature: Some(self.temperature.load(Ordering::Relaxed)),
            fault: self.fault.load(Ordering::Relaxed),
            last_updated: chrono::Utc::now(),
        })
    }

    async fn set_state(&mut self, state: RfDeviceState) -> Result<()> {
        self.enabled.store(state.enabled, Ordering::Relaxed);
        if let Some(temp) = state.temperature {
            self.temperature.store(temp, Ordering::Relaxed);
        }
        self.fault.store(state.fault, Ordering::Relaxed);
        Ok(())
    }

    async fn enable(&mut self) -> Result<()> {
        if self.fault.load(Ordering::Relaxed) {
            return Err(GroundStationError::Hardware("Amplifier fault - cannot enable".to_string()));
        }
        self.enabled.store(true, Ordering::Relaxed);
        let mut status = self.amplifier_status.write().await;
        *status = AmplifierStatus::Active;
        Ok(())
    }

    async fn disable(&mut self) -> Result<()> {
        self.enabled.store(false, Ordering::Relaxed);
        let mut status = self.amplifier_status.write().await;
        *status = AmplifierStatus::Idle;
        Ok(())
    }

    async fn get_calibration(&self) -> Result<DeviceCalibration> {
        Ok(DeviceCalibration {
            calibrated_at: chrono::Utc::now(),
            coefficients: vec![self.current_gain.load(Ordering::Relaxed)],
            notes: "Amplifier calibration".to_string(),
            valid_until: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.enabled.load(Ordering::Relaxed) && !self.fault.load(Ordering::Relaxed))
    }
}

#[async_trait::async_trait]
impl AmplifierControl for GenericAmplifier {
    async fn set_gain(&mut self, gain_db: f64) -> Result<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(GroundStationError::Hardware("Amplifier disabled".to_string()));
        }

        if self.fault.load(Ordering::Relaxed) {
            return Err(GroundStationError::Hardware("Amplifier fault".to_string()));
        }

        // Validate gain range (example: -20 to +60 dB)
        if gain_db < -20.0 || gain_db > 60.0 {
            return Err(GroundStationError::Validation("Gain out of range".to_string()));
        }

        self.current_gain.store(gain_db, Ordering::Relaxed);
        Ok(())
    }

    async fn get_gain(&self) -> Result<f64> {
        Ok(self.current_gain.load(Ordering::Relaxed))
    }

    async fn enable(&mut self) -> Result<()> {
        if self.fault.load(Ordering::Relaxed) {
            return Err(GroundStationError::Hardware("Amplifier fault - cannot enable".to_string()));
        }
        self.enabled.store(true, Ordering::Relaxed);
        let mut status = self.amplifier_status.write().await;
        *status = AmplifierStatus::Active;
        Ok(())
    }

    async fn disable(&mut self) -> Result<()> {
        self.enabled.store(false, Ordering::Relaxed);
        let mut status = self.amplifier_status.write().await;
        *status = AmplifierStatus::Idle;
        Ok(())
    }

    async fn get_temperature(&self) -> Result<f64> {
        Ok(self.temperature.load(Ordering::Relaxed))
    }

    async fn get_noise_figure(&self) -> Result<f64> {
        // Simplified - in real implementation would depend on gain and device
        Ok(2.0)
    }

    async fn get_amplifier_status(&self) -> Result<AmplifierStatus> {
        let status = self.amplifier_status.read().await;
        Ok(*status)
    }

    async fn configure_gain_stages(&mut self, stages: Vec<GainStage>) -> Result<()> {
        let mut gain_stages = self.gain_stages.write().await;
        *gain_stages = stages;
        Ok(())
    }

    async fn get_gain_stages(&self) -> Result<Vec<GainStage>> {
        let gain_stages = self.gain_stages.read().await;
        Ok(gain_stages.clone())
    }
}

/// Mini-Circuits amplifier adapter
pub struct MiniCircuitsAmplifier {
    inner: GenericAmplifier,
    gain_range: (f64, f64),
}

impl MiniCircuitsAmplifier {
    pub fn new(gain_min: f64, gain_max: f64, max_temperature: f64) -> Self {
        Self {
            inner: GenericAmplifier::new(RfDeviceType::Lna, max_temperature),
            gain_range: (gain_min, gain_max),
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for MiniCircuitsAmplifier {
    fn get_type(&self) -> RfDeviceType {
        self.inner.get_type()
    }

    async fn get_state(&self) -> Result<RfDeviceState> {
        self.inner.get_state().await
    }

    async fn set_state(&mut self, state: RfDeviceState) -> Result<()> {
        self.inner.set_state(state).await
    }

    async fn enable(&mut self) -> Result<()> {
        self.inner.enable().await
    }

    async fn disable(&mut self) -> Result<()> {
        self.inner.disable().await
    }

    async fn get_calibration(&self) -> Result<DeviceCalibration> {
        self.inner.get_calibration().await
    }

    async fn health_check(&self) -> Result<bool> {
        self.inner.health_check().await
    }
}

#[async_trait::async_trait]
impl AmplifierControl for MiniCircuitsAmplifier {
    async fn set_gain(&mut self, gain_db: f64) -> Result<()> {
        if gain_db < self.gain_range.0 || gain_db > self.gain_range.1 {
            return Err(GroundStationError::Validation(format!(
                "Gain {} out of range [{}, {}]",
                gain_db, self.gain_range.0, self.gain_range.1
            )));
        }
        self.inner.set_gain(gain_db).await
    }

    async fn get_gain(&self) -> Result<f64> {
        self.inner.get_gain().await
    }

    async fn enable(&mut self) -> Result<()> {
        self.inner.enable().await
    }

    async fn disable(&mut self) -> Result<()> {
        self.inner.disable().await
    }

    async fn get_temperature(&self) -> Result<f64> {
        self.inner.get_temperature().await
    }

    async fn get_noise_figure(&self) -> Result<f64> {
        self.inner.get_noise_figure().await
    }

    async fn get_amplifier_status(&self) -> Result<AmplifierStatus> {
        self.inner.get_amplifier_status().await
    }

    async fn configure_gain_stages(&mut self, stages: Vec<GainStage>) -> Result<()> {
        self.inner.configure_gain_stages(stages).await
    }

    async fn get_gain_stages(&self) -> Result<Vec<GainStage>> {
        self.inner.get_gain_stages().await
    }
}

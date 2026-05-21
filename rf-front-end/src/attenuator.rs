//! Attenuator control

use crate::device::{DeviceCalibration, RfDevice, RfDeviceState, RfDeviceType};
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Attenuation level
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttenuationLevel {
    /// Attenuation in dB
    pub db: f64,
    /// Step size in dB
    pub step_size: f64,
}

/// Attenuator control trait
#[async_trait::async_trait]
pub trait AttenuatorControl: RfDevice {
    /// Set attenuation in dB
    async fn set_attenuation(&mut self, attenuation_db: f64) -> Result<()>;

    /// Get current attenuation in dB
    async fn get_attenuation(&self) -> Result<f64>;

    /// Increment attenuation by step
    async fn increment_attenuation(&mut self, step_db: f64) -> Result<()>;

    /// Decrement attenuation by step
    async fn decrement_attenuation(&mut self, step_db: f64) -> Result<()>;

    /// Get attenuation range
    async fn get_attenuation_range(&self) -> Result<(f64, f64)>;

    /// Get step size
    async fn get_step_size(&self) -> Result<f64>;
}

/// Generic variable attenuator
pub struct VariableAttenuator {
    id: Uuid,
    enabled: bool,
    current_attenuation: RwLock<f64>,
    min_attenuation: f64,
    max_attenuation: f64,
    step_size: f64,
    calibration_table: RwLock<Vec<(f64, f64)>>, // (commanded_db, actual_db)
}

impl VariableAttenuator {
    /// Create a new variable attenuator
    pub fn new(min_attenuation: f64, max_attenuation: f64, step_size: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: false,
            current_attenuation: RwLock::new(0.0),
            min_attenuation,
            max_attenuation,
            step_size,
            calibration_table: RwLock::new(Vec::new()),
        }
    }

    /// Add calibration point
    pub async fn add_calibration_point(&self, commanded_db: f64, actual_db: f64) {
        let mut table = self.calibration_table.write().await;
        table.push((commanded_db, actual_db));
    }

    /// Get calibrated attenuation
    async fn get_calibrated_attenuation(&self, commanded_db: f64) -> f64 {
        let table = self.calibration_table.read().await;

        if table.is_empty() {
            return commanded_db;
        }

        // Linear interpolation between calibration points
        let mut lower = None;
        let mut upper = None;

        for &(cmd, actual) in table.iter() {
            if cmd <= commanded_db {
                lower = Some((cmd, actual));
            } else {
                upper = Some((cmd, actual));
                break;
            }
        }

        match (lower, upper) {
            (Some((lower_cmd, lower_actual)), Some((upper_cmd, upper_actual))) => {
                let ratio = (commanded_db - lower_cmd) / (upper_cmd - lower_cmd);
                lower_actual + ratio * (upper_actual - lower_actual)
            }
            (Some((_, actual)), None) => actual,
            (None, Some((_, actual))) => actual,
            _ => commanded_db,
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for VariableAttenuator {
    fn get_type(&self) -> RfDeviceType {
        RfDeviceType::Attenuator
    }

    async fn get_state(&self) -> Result<RfDeviceState> {
        let current = *self.current_attenuation.read().await;
        let calibrated = self.get_calibrated_attenuation(current).await;

        Ok(RfDeviceState {
            device_id: self.id,
            device_type: RfDeviceType::Attenuator,
            enabled: self.enabled,
            settings: serde_json::json!({
                "attenuation_db": current,
                "calibrated_attenuation_db": calibrated,
                "min_attenuation": self.min_attenuation,
                "max_attenuation": self.max_attenuation,
                "step_size": self.step_size,
            }),
            temperature: None,
            fault: false,
            last_updated: chrono::Utc::now(),
        })
    }

    async fn set_state(&mut self, state: RfDeviceState) -> Result<()> {
        self.enabled = state.enabled;
        Ok(())
    }

    async fn enable(&mut self) -> Result<()> {
        self.enabled = true;
        Ok(())
    }

    async fn disable(&mut self) -> Result<()> {
        self.enabled = false;
        Ok(())
    }

    async fn get_calibration(&self) -> Result<DeviceCalibration> {
        let table = self.calibration_table.read().await;
        let coefficients: Vec<f64> = table.iter().map(|(cmd, actual)| actual - cmd).collect();

        Ok(DeviceCalibration {
            calibrated_at: chrono::Utc::now(),
            coefficients,
            notes: "Attenuator calibration".to_string(),
            valid_until: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.enabled)
    }
}

#[async_trait::async_trait]
impl AttenuatorControl for VariableAttenuator {
    async fn set_attenuation(&mut self, attenuation_db: f64) -> Result<()> {
        if !self.enabled {
            return Err(GroundStationError::Hardware(
                "Attenuator disabled".to_string(),
            ));
        }

        if attenuation_db < self.min_attenuation || attenuation_db > self.max_attenuation {
            return Err(GroundStationError::Validation(format!(
                "Attenuation {} out of range [{}, {}]",
                attenuation_db, self.min_attenuation, self.max_attenuation
            )));
        }

        // Quantize to step size
        let quantized = (attenuation_db / self.step_size).round() * self.step_size;

        *self.current_attenuation.write().await = quantized;
        tracing::debug!(
            "Attenuator set to {} dB (quantized from {} dB)",
            quantized,
            attenuation_db
        );

        Ok(())
    }

    async fn get_attenuation(&self) -> Result<f64> {
        let current = *self.current_attenuation.read().await;
        let calibrated = self.get_calibrated_attenuation(current).await;
        Ok(calibrated)
    }

    async fn increment_attenuation(&mut self, step_db: f64) -> Result<()> {
        let current = *self.current_attenuation.read().await;
        let new_attenuation = current + step_db;
        self.set_attenuation(new_attenuation).await
    }

    async fn decrement_attenuation(&mut self, step_db: f64) -> Result<()> {
        let current = *self.current_attenuation.read().await;
        let new_attenuation = current - step_db;
        self.set_attenuation(new_attenuation).await
    }

    async fn get_attenuation_range(&self) -> Result<(f64, f64)> {
        Ok((self.min_attenuation, self.max_attenuation))
    }

    async fn get_step_size(&self) -> Result<f64> {
        Ok(self.step_size)
    }
}

/// Digital step attenuator
pub struct DigitalStepAttenuator {
    inner: VariableAttenuator,
    num_bits: usize,
}

impl DigitalStepAttenuator {
    /// Create a new digital step attenuator
    pub fn new(num_bits: usize, max_attenuation: f64) -> Self {
        let step_size = max_attenuation / (2_u64.pow(num_bits as u32) as f64);
        Self {
            inner: VariableAttenuator::new(0.0, max_attenuation, step_size),
            num_bits,
        }
    }

    /// Set attenuation using digital control word
    pub async fn set_control_word(&mut self, control_word: u64) -> Result<()> {
        if control_word >= (1_u64 << self.num_bits) {
            return Err(GroundStationError::Validation(
                "Control word out of range".to_string(),
            ));
        }

        let attenuation = (control_word as f64 / (2_u64.pow(self.num_bits as u32) as f64))
            * self.inner.max_attenuation;
        self.inner.set_attenuation(attenuation).await
    }

    /// Get current control word
    pub async fn get_control_word(&self) -> Result<u64> {
        let current = *self.inner.current_attenuation.read().await;
        let ratio = current / self.inner.max_attenuation;
        let control_word = (ratio * (2_u64.pow(self.num_bits as u32) as f64)) as u64;
        Ok(control_word)
    }
}

#[async_trait::async_trait]
impl RfDevice for DigitalStepAttenuator {
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
impl AttenuatorControl for DigitalStepAttenuator {
    async fn set_attenuation(&mut self, attenuation_db: f64) -> Result<()> {
        self.inner.set_attenuation(attenuation_db).await
    }

    async fn get_attenuation(&self) -> Result<f64> {
        self.inner.get_attenuation().await
    }

    async fn increment_attenuation(&mut self, step_db: f64) -> Result<()> {
        self.inner.increment_attenuation(step_db).await
    }

    async fn decrement_attenuation(&mut self, step_db: f64) -> Result<()> {
        self.inner.decrement_attenuation(step_db).await
    }

    async fn get_attenuation_range(&self) -> Result<(f64, f64)> {
        self.inner.get_attenuation_range().await
    }

    async fn get_step_size(&self) -> Result<f64> {
        self.inner.get_step_size().await
    }
}

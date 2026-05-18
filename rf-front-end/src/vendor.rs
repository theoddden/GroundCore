//! Vendor-specific RF device adapters

use crate::amplifier::{AmplifierControl, GainStage, AmplifierStatus, GenericAmplifier};
use crate::filter::{FilterControl, FilterSpec, FilterId, FilterType, FilterBank};
use crate::attenuator::{AttenuatorControl, VariableAttenuator};
use crate::device::{RfDevice, RfDeviceType, RfDeviceState, DeviceCalibration};
use async_trait::async_trait;
use ground_core::{Result, GroundStationError};
use std::time::Duration;
use tokio::time::sleep;

/// Mini-Circuits amplifier adapter
pub struct MiniCircuitsAmplifierAdapter {
    inner: GenericAmplifier,
    gain_range: (f64, f64),
}

impl MiniCircuitsAmplifierAdapter {
    pub fn new(gain_min: f64, gain_max: f64, max_temperature: f64) -> Self {
        Self {
            inner: GenericAmplifier::new(RfDeviceType::Lna, max_temperature),
            gain_range: (gain_min, gain_max),
        }
    }

    /// Send command via USB (simulated)
    async fn send_usb_command(&self, command: &[u8]) -> Result<Vec<u8>> {
        // In real implementation, this would communicate via USB
        tracing::debug!("Mini-Circuits USB command: {:?}", command);
        sleep(Duration::from_millis(10)).await;
        Ok(vec![0x00]) // Simulate ACK
    }
}

#[async_trait::async_trait]
impl RfDevice for MiniCircuitsAmplifierAdapter {
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
impl AmplifierControl for MiniCircuitsAmplifierAdapter {
    async fn set_gain(&mut self, gain_db: f64) -> Result<()> {
        if gain_db < self.gain_range.0 || gain_db > self.gain_range.1 {
            return Err(GroundStationError::Validation(format!(
                "Gain {} out of range [{}, {}]",
                gain_db, self.gain_range.0, self.gain_range.1
            )));
        }

        // Format Mini-Circuits command (simplified)
        let cmd = format!("GAIN {:.2}\n", gain_db);
        self.send_usb_command(cmd.as_bytes()).await?;

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

/// Qorvo amplifier adapter
pub struct QorvoAmplifierAdapter {
    inner: GenericAmplifier,
    gain_range: (f64, f64),
}

impl QorvoAmplifierAdapter {
    pub fn new(gain_min: f64, gain_max: f64, max_temperature: f64) -> Self {
        Self {
            inner: GenericAmplifier::new(RfDeviceType::PowerAmplifier, max_temperature),
            gain_range: (gain_min, gain_max),
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for QorvoAmplifierAdapter {
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
impl AmplifierControl for QorvoAmplifierAdapter {
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

/// Crystek filter bank adapter
pub struct CrystekFilterBankAdapter {
    inner: FilterBank,
}

impl CrystekFilterBankAdapter {
    pub fn new() -> Self {
        Self {
            inner: FilterBank::new(),
        }
    }

    /// Add standard Crystek filters
    pub async fn add_standard_filters(&mut self) -> Result<()> {
        let filters = vec![
            FilterSpec {
                id: uuid::Uuid::new_v4(),
                filter_type: FilterType::Bandpass,
                center_frequency: 1_500_000_000, // 1.5 GHz
                bandwidth: 50_000_000, // 50 MHz
                insertion_loss: 2.0,
                vswr: 1.5,
                tunable: false,
                min_frequency: None,
                max_frequency: None,
            },
            FilterSpec {
                id: uuid::Uuid::new_v4(),
                filter_type: FilterType::Bandpass,
                center_frequency: 2_400_000_000, // 2.4 GHz
                bandwidth: 50_000_000,
                insertion_loss: 2.0,
                vswr: 1.5,
                tunable: false,
                min_frequency: None,
                max_frequency: None,
            },
            FilterSpec {
                id: uuid::Uuid::new_v4(),
                filter_type: FilterType::Bandpass,
                center_frequency: 8_400_000_000, // 8.4 GHz (X-band)
                bandwidth: 100_000_000,
                insertion_loss: 2.5,
                vswr: 1.5,
                tunable: false,
                min_frequency: None,
                max_frequency: None,
            },
        ];

        for filter in filters {
            self.inner.add_filter(filter).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl RfDevice for CrystekFilterBankAdapter {
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
impl FilterControl for CrystekFilterBankAdapter {
    async fn select_filter(&mut self, filter_id: FilterId) -> Result<()> {
        self.inner.select_filter(filter_id).await
    }

    async fn tune_filter(&mut self, frequency: u64) -> Result<()> {
        self.inner.tune_filter(frequency).await
    }

    async fn get_current_filter(&self) -> Result<FilterSpec> {
        self.inner.get_current_filter().await
    }

    async fn get_available_filters(&self) -> Result<Vec<FilterSpec>> {
        self.inner.get_available_filters().await
    }

    async fn add_filter(&mut self, filter: FilterSpec) -> Result<()> {
        self.inner.add_filter(filter).await
    }

    async fn remove_filter(&mut self, filter_id: FilterId) -> Result<()> {
        self.inner.remove_filter(filter_id).await
    }
}

/// Hittite attenuator adapter
pub struct HittiteAttenuatorAdapter {
    inner: VariableAttenuator,
}

impl HittiteAttenuatorAdapter {
    pub fn new(min_attenuation: f64, max_attenuation: f64, step_size: f64) -> Self {
        Self {
            inner: VariableAttenuator::new(min_attenuation, max_attenuation, step_size),
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for HittiteAttenuatorAdapter {
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
impl AttenuatorControl for HittiteAttenuatorAdapter {
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

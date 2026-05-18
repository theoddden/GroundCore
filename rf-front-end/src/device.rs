//! RF device abstraction

use ground_core::{Result, GroundStationError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// RF device identifier
pub type DeviceId = Uuid;

/// RF device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RfDeviceType {
    /// Low-noise amplifier
    Lna,
    /// Power amplifier
    PowerAmplifier,
    /// Bandpass filter
    BandpassFilter,
    /// Switchable filter bank
    FilterBank,
    /// RF switch
    Switch,
    /// Variable attenuator
    Attenuator,
    /// Mixer
    Mixer,
    /// Frequency converter
    FrequencyConverter,
}

/// RF device state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfDeviceState {
    /// Device ID
    pub device_id: DeviceId,
    /// Device type
    pub device_type: RfDeviceType,
    /// Enabled flag
    pub enabled: bool,
    /// Current settings (device-specific)
    pub settings: serde_json::Value,
    /// Temperature (Celsius)
    pub temperature: Option<f64>,
    /// Fault flag
    pub fault: bool,
    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// RF device trait
#[async_trait::async_trait]
pub trait RfDevice: Send + Sync {
    /// Get device type
    fn get_type(&self) -> RfDeviceType;
    
    /// Get device state
    async fn get_state(&self) -> Result<RfDeviceState>;
    
    /// Set device state
    async fn set_state(&mut self, state: RfDeviceState) -> Result<()>;
    
    /// Enable device
    async fn enable(&mut self) -> Result<()>;
    
    /// Disable device
    async fn disable(&mut self) -> Result<()>;
    
    /// Get calibration data
    async fn get_calibration(&self) -> Result<DeviceCalibration>;
    
    /// Check if device is healthy
    async fn health_check(&self) -> Result<bool>;
}

/// Device calibration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCalibration {
    /// Calibration date
    pub calibrated_at: chrono::DateTime<chrono::Utc>,
    /// Calibration coefficients
    pub coefficients: Vec<f64>,
    /// Calibration notes
    pub notes: String,
    /// Calibration valid until
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

/// Generic RF device implementation
pub struct GenericRfDevice {
    id: DeviceId,
    device_type: RfDeviceType,
    enabled: bool,
    temperature: Option<f64>,
    fault: bool,
    settings: serde_json::Value,
}

impl GenericRfDevice {
    /// Create a new generic RF device
    pub fn new(device_type: RfDeviceType) -> Self {
        Self {
            id: Uuid::new_v4(),
            device_type,
            enabled: false,
            temperature: None,
            fault: false,
            settings: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for GenericRfDevice {
    fn get_type(&self) -> RfDeviceType {
        self.device_type
    }
    
    async fn get_state(&self) -> Result<RfDeviceState> {
        Ok(RfDeviceState {
            device_id: self.id,
            device_type: self.device_type,
            enabled: self.enabled,
            settings: self.settings.clone(),
            temperature: self.temperature,
            fault: self.fault,
            last_updated: chrono::Utc::now(),
        })
    }
    
    async fn set_state(&mut self, state: RfDeviceState) -> Result<()> {
        self.enabled = state.enabled;
        self.settings = state.settings;
        self.temperature = state.temperature;
        self.fault = state.fault;
        Ok(())
    }
    
    async fn enable(&mut self) -> Result<()> {
        if self.fault {
            return Err(GroundStationError::Hardware("Device fault - cannot enable".to_string()));
        }
        self.enabled = true;
        Ok(())
    }
    
    async fn disable(&mut self) -> Result<()> {
        self.enabled = false;
        Ok(())
    }
    
    async fn get_calibration(&self) -> Result<DeviceCalibration> {
        Ok(DeviceCalibration {
            calibrated_at: chrono::Utc::now(),
            coefficients: Vec::new(),
            notes: "Default calibration".to_string(),
            valid_until: None,
        })
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Check if device is enabled and not faulted
        Ok(self.enabled && !self.fault)
    }
}

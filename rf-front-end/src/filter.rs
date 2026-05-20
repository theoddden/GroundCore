//! Filter control with switching and tuning

use crate::device::{DeviceCalibration, RfDevice, RfDeviceState, RfDeviceType};
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Filter identifier
pub type FilterId = Uuid;

/// Filter type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    /// Bandpass filter
    Bandpass,
    /// Lowpass filter
    Lowpass,
    /// Highpass filter
    Highpass,
    /// Notch filter
    Notch,
}

/// Filter specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterSpec {
    /// Filter ID
    pub id: FilterId,
    /// Filter type
    pub filter_type: FilterType,
    /// Center frequency (Hz)
    pub center_frequency: u64,
    /// Bandwidth (Hz)
    pub bandwidth: u64,
    /// Insertion loss (dB)
    pub insertion_loss: f64,
    /// VSWR (Voltage Standing Wave Ratio)
    pub vswr: f64,
    /// Tunable flag
    pub tunable: bool,
    /// Minimum frequency (for tunable filters)
    pub min_frequency: Option<u64>,
    /// Maximum frequency (for tunable filters)
    pub max_frequency: Option<u64>,
}

/// Filter control trait
#[async_trait::async_trait]
pub trait FilterControl: RfDevice {
    /// Select a filter
    async fn select_filter(&mut self, filter_id: FilterId) -> Result<()>;

    /// Tune filter to specific frequency (for tunable filters)
    async fn tune_filter(&mut self, frequency: u64) -> Result<()>;

    /// Get current filter
    async fn get_current_filter(&self) -> Result<FilterSpec>;

    /// Get available filters
    async fn get_available_filters(&self) -> Result<Vec<FilterSpec>>;

    /// Add a filter to the bank
    async fn add_filter(&mut self, filter: FilterSpec) -> Result<()>;

    /// Remove a filter from the bank
    async fn remove_filter(&mut self, filter_id: FilterId) -> Result<()>;
}

/// Switchable filter bank
pub struct FilterBank {
    id: Uuid,
    enabled: bool,
    current_filter: RwLock<Option<FilterId>>,
    filters: RwLock<HashMap<FilterId, FilterSpec>>,
}

impl FilterBank {
    /// Create a new filter bank
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: false,
            current_filter: RwLock::new(None),
            filters: RwLock::new(HashMap::new()),
        }
    }

    /// Add a filter to the bank
    pub async fn add_filter_internal(&self, filter: FilterSpec) {
        let mut filters = self.filters.write().await;
        filters.insert(filter.id, filter);
    }
}

#[async_trait::async_trait]
impl RfDevice for FilterBank {
    fn get_type(&self) -> RfDeviceType {
        RfDeviceType::FilterBank
    }

    async fn get_state(&self) -> Result<RfDeviceState> {
        let current = self.current_filter.read().await;
        let filters = self.filters.read().await;

        Ok(RfDeviceState {
            device_id: self.id,
            device_type: RfDeviceType::FilterBank,
            enabled: self.enabled,
            settings: serde_json::json!({
                "current_filter": *current,
                "filter_count": filters.len(),
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
        Ok(DeviceCalibration {
            calibrated_at: chrono::Utc::now(),
            coefficients: Vec::new(),
            notes: "Filter bank calibration".to_string(),
            valid_until: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.enabled)
    }
}

#[async_trait::async_trait]
impl FilterControl for FilterBank {
    async fn select_filter(&mut self, filter_id: FilterId) -> Result<()> {
        if !self.enabled {
            return Err(GroundStationError::Hardware(
                "Filter bank disabled".to_string(),
            ));
        }

        let filters = self.filters.read().await;
        if !filters.contains_key(&filter_id) {
            return Err(GroundStationError::NotFound(format!(
                "Filter {}",
                filter_id
            )));
        }
        drop(filters);

        let mut current = self.current_filter.write().await;
        *current = Some(filter_id);

        tracing::info!("Selected filter {}", filter_id);
        Ok(())
    }

    async fn tune_filter(&mut self, frequency: u64) -> Result<()> {
        let current = self.current_filter.read().await;
        let filter_id = current
            .ok_or_else(|| GroundStationError::Hardware("No filter selected".to_string()))?;
        drop(current);

        let filters = self.filters.read().await;
        let filter = filters
            .get(&filter_id)
            .ok_or_else(|| GroundStationError::NotFound(format!("Filter {}", filter_id)))?;

        if !filter.tunable {
            return Err(GroundStationError::Hardware(
                "Filter is not tunable".to_string(),
            ));
        }

        if let Some(min) = filter.min_frequency {
            if frequency < min {
                return Err(GroundStationError::Validation(
                    "Frequency below minimum".to_string(),
                ));
            }
        }

        if let Some(max) = filter.max_frequency {
            if frequency > max {
                return Err(GroundStationError::Validation(
                    "Frequency above maximum".to_string(),
                ));
            }
        }

        // In real implementation, send tuning command to hardware
        tracing::info!("Tuned filter {} to {} Hz", filter_id, frequency);
        Ok(())
    }

    async fn get_current_filter(&self) -> Result<FilterSpec> {
        let current = self.current_filter.read().await;
        let filter_id = current
            .ok_or_else(|| GroundStationError::Hardware("No filter selected".to_string()))?;
        drop(current);

        let filters = self.filters.read().await;
        filters
            .get(&filter_id)
            .cloned()
            .ok_or_else(|| GroundStationError::NotFound(format!("Filter {}", filter_id)))
    }

    async fn get_available_filters(&self) -> Result<Vec<FilterSpec>> {
        let filters = self.filters.read().await;
        Ok(filters.values().cloned().collect())
    }

    async fn add_filter(&mut self, filter: FilterSpec) -> Result<()> {
        let mut filters = self.filters.write().await;
        filters.insert(filter.id, filter);
        Ok(())
    }

    async fn remove_filter(&mut self, filter_id: FilterId) -> Result<()> {
        let mut filters = self.filters.write().await;
        filters.remove(&filter_id);

        // Clear current if it was the removed filter
        let mut current = self.current_filter.write().await;
        if *current == Some(filter_id) {
            *current = None;
        }

        Ok(())
    }
}

/// YIG-tuned filter (Yttrium Iron Garnet)
pub struct YigTunedFilter {
    id: Uuid,
    enabled: bool,
    current_frequency: RwLock<u64>,
    min_frequency: u64,
    max_frequency: u64,
    tuning_voltage: RwLock<f64>,
}

impl YigTunedFilter {
    /// Create a new YIG-tuned filter
    pub fn new(min_frequency: u64, max_frequency: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: false,
            current_frequency: RwLock::new(min_frequency),
            min_frequency,
            max_frequency,
            tuning_voltage: RwLock::new(0.0),
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for YigTunedFilter {
    fn get_type(&self) -> RfDeviceType {
        RfDeviceType::BandpassFilter
    }

    async fn get_state(&self) -> Result<RfDeviceState> {
        let current_frequency = self.current_frequency.read().await;
        let tuning_voltage = self.tuning_voltage.read().await;

        Ok(RfDeviceState {
            device_id: self.id,
            device_type: RfDeviceType::BandpassFilter,
            enabled: self.enabled,
            settings: serde_json::json!({
                "current_frequency": *current_frequency,
                "tuning_voltage": *tuning_voltage,
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
        Ok(DeviceCalibration {
            calibrated_at: chrono::Utc::now(),
            coefficients: Vec::new(),
            notes: "YIG filter calibration".to_string(),
            valid_until: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.enabled)
    }
}

#[async_trait::async_trait]
impl FilterControl for YigTunedFilter {
    async fn select_filter(&mut self, _filter_id: FilterId) -> Result<()> {
        // YIG filters don't have selectable filters - they're continuously tunable
        Err(GroundStationError::Hardware(
            "YIG filter is continuously tunable, not switchable".to_string(),
        ))
    }

    async fn tune_filter(&mut self, frequency: u64) -> Result<()> {
        if !self.enabled {
            return Err(GroundStationError::Hardware("Filter disabled".to_string()));
        }

        if frequency < self.min_frequency || frequency > self.max_frequency {
            return Err(GroundStationError::Validation(format!(
                "Frequency {} out of range [{}, {}]",
                frequency, self.min_frequency, self.max_frequency
            )));
        }

        // Calculate tuning voltage (simplified linear relationship)
        let voltage = ((frequency - self.min_frequency) as f64
            / (self.max_frequency - self.min_frequency) as f64)
            * 10.0;

        let mut current_frequency = self.current_frequency.write().await;
        *current_frequency = frequency;

        let mut tuning_voltage = self.tuning_voltage.write().await;
        *tuning_voltage = voltage;

        tracing::info!(
            "YIG filter tuned to {} Hz (voltage: {} V)",
            frequency,
            voltage
        );
        Ok(())
    }

    async fn get_current_filter(&self) -> Result<FilterSpec> {
        let current_frequency = self.current_frequency.read().await;
        Ok(FilterSpec {
            id: self.id,
            filter_type: FilterType::Bandpass,
            center_frequency: *current_frequency,
            bandwidth: 10_000_000, // Example 10 MHz bandwidth
            insertion_loss: 2.0,
            vswr: 1.5,
            tunable: true,
            min_frequency: Some(self.min_frequency),
            max_frequency: Some(self.max_frequency),
        })
    }

    async fn get_available_filters(&self) -> Result<Vec<FilterSpec>> {
        // YIG filter has continuous range, not discrete filters
        Ok(vec![])
    }

    async fn add_filter(&mut self, _filter: FilterSpec) -> Result<()> {
        Err(GroundStationError::Hardware(
            "YIG filter is continuously tunable, cannot add filters".to_string(),
        ))
    }

    async fn remove_filter(&mut self, _filter_id: FilterId) -> Result<()> {
        Err(GroundStationError::Hardware(
            "YIG filter is continuously tunable, cannot remove filters".to_string(),
        ))
    }
}

//! RF switch control

use crate::device::{DeviceCalibration, RfDevice, RfDeviceState, RfDeviceType};
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Switch position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwitchPosition {
    Position1,
    Position2,
    Position3,
    Position4,
    Position5,
    Position6,
}

/// Switch state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwitchState {
    Open,
    Closed,
}

/// Switch type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwitchType {
    /// Antenna switch
    Antenna,
    /// Transmit/receive switch
    TransmitReceive,
    /// Polarization switch
    Polarization,
    /// Band switch
    Band,
}

/// Switch control trait
#[async_trait::async_trait]
pub trait SwitchControl: RfDevice {
    /// Set switch position
    async fn set_position(&mut self, position: SwitchPosition) -> Result<()>;

    /// Get current switch position
    async fn get_position(&self) -> Result<SwitchPosition>;

    /// Get available positions
    async fn get_available_positions(&self) -> Result<Vec<SwitchPosition>>;

    /// Set switch state (for individual switch in a bank)
    async fn set_switch_state(&mut self, switch_index: usize, state: SwitchState) -> Result<()>;

    /// Get switch state
    async fn get_switch_state(&self, switch_index: usize) -> Result<SwitchState>;
}

/// Generic RF switch
pub struct GenericRfSwitch {
    id: Uuid,
    switch_type: SwitchType,
    enabled: bool,
    current_position: RwLock<SwitchPosition>,
    available_positions: Vec<SwitchPosition>,
    switch_states: RwLock<Vec<SwitchState>>,
}

impl GenericRfSwitch {
    /// Create a new generic RF switch
    pub fn new(
        switch_type: SwitchType,
        available_positions: Vec<SwitchPosition>,
        num_switches: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            switch_type,
            enabled: false,
            current_position: RwLock::new(available_positions[0]),
            available_positions,
            switch_states: RwLock::new(vec![SwitchState::Open; num_switches]),
        }
    }
}

#[async_trait::async_trait]
impl RfDevice for GenericRfSwitch {
    fn get_type(&self) -> RfDeviceType {
        RfDeviceType::Switch
    }

    async fn get_state(&self) -> Result<RfDeviceState> {
        let position = self.current_position.read().await;
        let states = self.switch_states.read().await;

        Ok(RfDeviceState {
            device_id: self.id,
            device_type: RfDeviceType::Switch,
            enabled: self.enabled,
            settings: serde_json::json!({
                "switch_type": self.switch_type,
                "current_position": *position,
                "switch_states": *states,
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
            notes: "Switch calibration".to_string(),
            valid_until: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.enabled)
    }
}

#[async_trait::async_trait]
impl SwitchControl for GenericRfSwitch {
    async fn set_position(&mut self, position: SwitchPosition) -> Result<()> {
        if !self.enabled {
            return Err(GroundStationError::Hardware("Switch disabled".to_string()));
        }

        if !self.available_positions.contains(&position) {
            return Err(GroundStationError::Validation(
                "Invalid switch position".to_string(),
            ));
        }

        let mut current = self.current_position.write().await;
        *current = position;

        tracing::info!("Switch set to position {:?}", position);
        Ok(())
    }

    async fn get_position(&self) -> Result<SwitchPosition> {
        let current = self.current_position.read().await;
        Ok(*current)
    }

    async fn get_available_positions(&self) -> Result<Vec<SwitchPosition>> {
        Ok(self.available_positions.clone())
    }

    async fn set_switch_state(&mut self, switch_index: usize, state: SwitchState) -> Result<()> {
        if !self.enabled {
            return Err(GroundStationError::Hardware("Switch disabled".to_string()));
        }

        let mut states = self.switch_states.write().await;
        if switch_index >= states.len() {
            return Err(GroundStationError::Validation(
                "Invalid switch index".to_string(),
            ));
        }

        states[switch_index] = state;
        Ok(())
    }

    async fn get_switch_state(&self, switch_index: usize) -> Result<SwitchState> {
        let states = self.switch_states.read().await;
        if switch_index >= states.len() {
            return Err(GroundStationError::Validation(
                "Invalid switch index".to_string(),
            ));
        }
        Ok(states[switch_index])
    }
}

/// Transmit/receive switch (T/R switch)
pub struct TransmitReceiveSwitch {
    inner: GenericRfSwitch,
}

impl TransmitReceiveSwitch {
    /// Create a new T/R switch
    pub fn new() -> Self {
        Self {
            inner: GenericRfSwitch::new(
                SwitchType::TransmitReceive,
                vec![SwitchPosition::Position1, SwitchPosition::Position2],
                2,
            ),
        }
    }

    /// Switch to transmit mode
    pub async fn switch_to_transmit(&mut self) -> Result<()> {
        self.inner.set_position(SwitchPosition::Position1).await
    }

    /// Switch to receive mode
    pub async fn switch_to_receive(&mut self) -> Result<()> {
        self.inner.set_position(SwitchPosition::Position2).await
    }

    /// Check if in transmit mode
    pub async fn is_transmit(&self) -> Result<bool> {
        let position = self.inner.get_position().await?;
        Ok(position == SwitchPosition::Position1)
    }

    /// Check if in receive mode
    pub async fn is_receive(&self) -> Result<bool> {
        let position = self.inner.get_position().await?;
        Ok(position == SwitchPosition::Position2)
    }
}

#[async_trait::async_trait]
impl RfDevice for TransmitReceiveSwitch {
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
impl SwitchControl for TransmitReceiveSwitch {
    async fn set_position(&mut self, position: SwitchPosition) -> Result<()> {
        self.inner.set_position(position).await
    }

    async fn get_position(&self) -> Result<SwitchPosition> {
        self.inner.get_position().await
    }

    async fn get_available_positions(&self) -> Result<Vec<SwitchPosition>> {
        self.inner.get_available_positions().await
    }

    async fn set_switch_state(&mut self, switch_index: usize, state: SwitchState) -> Result<()> {
        self.inner.set_switch_state(switch_index, state).await
    }

    async fn get_switch_state(&self, switch_index: usize) -> Result<SwitchState> {
        self.inner.get_switch_state(switch_index).await
    }
}

/// Polarization switch
pub struct PolarizationSwitch {
    inner: GenericRfSwitch,
}

impl PolarizationSwitch {
    /// Create a new polarization switch
    pub fn new() -> Self {
        Self {
            inner: GenericRfSwitch::new(
                SwitchType::Polarization,
                vec![SwitchPosition::Position1, SwitchPosition::Position2],
                2,
            ),
        }
    }

    /// Switch to left-hand circular polarization (LHCP)
    pub async fn switch_to_lhcp(&mut self) -> Result<()> {
        self.inner.set_position(SwitchPosition::Position1).await
    }

    /// Switch to right-hand circular polarization (RHCP)
    pub async fn switch_to_rhcp(&mut self) -> Result<()> {
        self.inner.set_position(SwitchPosition::Position2).await
    }

    /// Switch to horizontal polarization
    pub async fn switch_to_horizontal(&mut self) -> Result<()> {
        self.inner.set_position(SwitchPosition::Position1).await
    }

    /// Switch to vertical polarization
    pub async fn switch_to_vertical(&mut self) -> Result<()> {
        self.inner.set_position(SwitchPosition::Position2).await
    }
}

#[async_trait::async_trait]
impl RfDevice for PolarizationSwitch {
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
impl SwitchControl for PolarizationSwitch {
    async fn set_position(&mut self, position: SwitchPosition) -> Result<()> {
        self.inner.set_position(position).await
    }

    async fn get_position(&self) -> Result<SwitchPosition> {
        self.inner.get_position().await
    }

    async fn get_available_positions(&self) -> Result<Vec<SwitchPosition>> {
        self.inner.get_available_positions().await
    }

    async fn set_switch_state(&mut self, switch_index: usize, state: SwitchState) -> Result<()> {
        self.inner.set_switch_state(switch_index, state).await
    }

    async fn get_switch_state(&self, switch_index: usize) -> Result<SwitchState> {
        self.inner.get_switch_state(switch_index).await
    }
}

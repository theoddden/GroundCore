//! Hardware pool management
//!
//! Manages the pool of available hardware (SDRs, antennas, rotators)
//! and allocates them to passes based on scheduling decisions.

use chrono::{DateTime, Duration, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Consecutive failures before the circuit opens
const CIRCUIT_OPEN_THRESHOLD: u32 = 3;
/// Seconds the circuit stays open before allowing a probe (half-open)
const CIRCUIT_COOLDOWN_SECS: i64 = 60;

/// Circuit breaker state for a hardware device
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Device healthy — allocations allowed
    Closed,
    /// Device failed repeatedly — allocations blocked until cooldown expires
    Open { opened_at: DateTime<Utc> },
    /// Cooldown elapsed — one probe allocation allowed to test recovery
    HalfOpen,
}

/// Per-device circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub total_trips: u32,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            total_trips: 0,
        }
    }

    /// Record a failure. Opens the circuit after CIRCUIT_OPEN_THRESHOLD failures.
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= CIRCUIT_OPEN_THRESHOLD {
            self.state = CircuitState::Open { opened_at: Utc::now() };
            self.total_trips += 1;
            tracing::warn!(
                "Circuit breaker opened after {} consecutive failures (trip #{})",
                self.consecutive_failures, self.total_trips
            );
        }
    }

    /// Record a success. Resets consecutive failures and closes the circuit.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.state = CircuitState::Closed;
    }

    /// Whether an allocation attempt is permitted right now.
    pub fn allows_allocation(&mut self) -> bool {
        match &self.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open { opened_at } => {
                let elapsed = (Utc::now() - *opened_at).num_seconds();
                if elapsed >= CIRCUIT_COOLDOWN_SECS {
                    tracing::info!("Circuit breaker entering half-open after {}s cooldown", elapsed);
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// Hardware resource type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardwareType {
    /// Software Defined Radio
    Sdr,
    /// Antenna rotator
    Rotator,
    /// Antenna
    Antenna,
    /// Amplifier
    Amplifier,
}

/// Hardware device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareDevice {
    /// Unique device identifier
    pub device_id: String,
    /// Device type
    pub hardware_type: HardwareType,
    /// Whether the device is healthy
    pub healthy: bool,
    /// Current allocation (if any)
    pub current_allocation: Option<PassId>,
    /// Device capabilities
    pub capabilities: HardwareCapabilities,
    /// Last health check
    pub last_health_check: DateTime<Utc>,
}

impl HardwareDevice {
    pub fn new(device_id: String, hardware_type: HardwareType, capabilities: HardwareCapabilities) -> Self {
        Self {
            device_id,
            hardware_type,
            healthy: true,
            current_allocation: None,
            capabilities,
            last_health_check: Utc::now(),
        }
    }
    
    /// Check if device is available for allocation
    pub fn is_available(&self) -> bool {
        self.healthy && self.current_allocation.is_none()
    }
    
    /// Allocate this device to a pass
    pub fn allocate(&mut self, pass_id: PassId) -> Result<()> {
        if !self.is_available() {
            return Err(ground_core::GroundStationError::Hardware(
                format!("Device {} not available", self.device_id),
            ));
        }
        
        self.current_allocation = Some(pass_id);
        Ok(())
    }
    
    /// Release this device from a pass
    pub fn release(&mut self, pass_id: &PassId) -> Result<()> {
        if self.current_allocation.as_ref() != Some(pass_id) {
            return Err(ground_core::GroundStationError::Hardware(
                format!("Device {} not allocated to pass {}", self.device_id, pass_id),
            ));
        }
        
        self.current_allocation = None;
        Ok(())
    }
}

/// Hardware capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    /// Frequency range (min, max) in Hz
    pub frequency_range: (u64, u64),
    /// Maximum sample rate in Hz
    pub max_sample_rate: u64,
    /// Supported bands
    pub supported_bands: Vec<ground_core::FrequencyBand>,
    /// Whether device supports transmit (vs receive-only)
    pub can_transmit: bool,
}

impl HardwareCapabilities {
    pub fn new(frequency_range: (u64, u64), max_sample_rate: u64) -> Self {
        Self {
            frequency_range,
            max_sample_rate,
            supported_bands: Vec::new(),
            can_transmit: false,
        }
    }
}

/// Hardware allocation for a pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAllocation {
    /// Pass ID
    pub pass_id: PassId,
    /// Allocated devices
    pub devices: Vec<String>,
    /// When allocation was made
    pub allocated_at: DateTime<Utc>,
    /// Expected duration
    pub expected_duration_sec: u64,
}

/// Hardware pool
pub struct HardwarePool {
    /// All devices in the pool
    devices: Arc<RwLock<HashMap<String, HardwareDevice>>>,
    /// Active allocations
    allocations: Arc<RwLock<HashMap<PassId, HardwareAllocation>>>,
    /// Per-device circuit breakers
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
}

impl HardwarePool {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add a device to the pool
    pub async fn add_device(&self, device: HardwareDevice) {
        let mut devices = self.devices.write().await;
        devices.insert(device.device_id.clone(), device);
    }
    
    /// Get a device by ID
    pub async fn get_device(&self, device_id: &str) -> Option<HardwareDevice> {
        let devices = self.devices.read().await;
        devices.get(device_id).cloned()
    }
    
    /// Find available devices of a specific type, excluding devices with open circuits.
    pub async fn find_available(&self, hardware_type: HardwareType, count: usize) -> Vec<String> {
        let devices = self.devices.read().await;
        let mut breakers = self.circuit_breakers.write().await;

        devices
            .values()
            .filter(|d| {
                if d.hardware_type != hardware_type || !d.is_available() {
                    return false;
                }
                // Check circuit breaker — mutable because allows_allocation() transitions state
                let breaker = breakers
                    .entry(d.device_id.clone())
                    .or_insert_with(CircuitBreaker::new);
                breaker.allows_allocation()
            })
            .take(count)
            .map(|d| d.device_id.clone())
            .collect()
    }
    
    /// Allocate hardware for a pass
    pub async fn allocate(&self, pass_id: PassId, device_ids: Vec<String>, duration_sec: u64) -> Result<HardwareAllocation> {
        let mut devices = self.devices.write().await;
        
        // Allocate each device
        for device_id in &device_ids {
            let device = devices.get_mut(device_id)
                .ok_or_else(|| ground_core::GroundStationError::Hardware(
                    format!("Device {} not found", device_id),
                ))?;
            
            device.allocate(pass_id.clone())?;
        }
        
        let allocation = HardwareAllocation {
            pass_id: pass_id.clone(),
            devices: device_ids.clone(),
            allocated_at: Utc::now(),
            expected_duration_sec: duration_sec,
        };
        
        let mut allocations = self.allocations.write().await;
        allocations.insert(pass_id, allocation.clone());
        
        Ok(allocation)
    }
    
    /// Release hardware from a pass
    pub async fn release(&self, pass_id: &PassId) -> Result<()> {
        let allocation = {
            let allocations = self.allocations.read().await;
            allocations.get(pass_id).cloned()
                .ok_or_else(|| ground_core::GroundStationError::Hardware(
                    format!("No allocation for pass {}", pass_id),
                ))?
        };
        
        let mut devices = self.devices.write().await;
        for device_id in &allocation.devices {
            if let Some(device) = devices.get_mut(device_id) {
                let _ = device.release(pass_id); // Ignore errors on release
            }
        }
        
        let mut allocations = self.allocations.write().await;
        allocations.remove(pass_id);
        
        Ok(())
    }
    
    /// Get current allocation for a pass
    pub async fn get_allocation(&self, pass_id: &PassId) -> Option<HardwareAllocation> {
        let allocations = self.allocations.read().await;
        allocations.get(pass_id).cloned()
    }
    
    /// Check device health
    pub async fn check_health(&self, device_id: &str) -> Result<bool> {
        let mut devices = self.devices.write().await;
        let device = devices.get_mut(device_id)
            .ok_or_else(|| ground_core::GroundStationError::Hardware(
                format!("Device {} not found", device_id),
            ))?;
        
        // In a real implementation, this would perform actual health checks
        // For now, we just update the timestamp
        device.last_health_check = Utc::now();
        Ok(device.healthy)
    }
    
    /// Mark a device as unhealthy and trip its circuit breaker.
    pub async fn mark_unhealthy(&self, device_id: &str) -> Result<()> {
        let mut devices = self.devices.write().await;
        let device = devices.get_mut(device_id)
            .ok_or_else(|| ground_core::GroundStationError::Hardware(
                format!("Device {} not found", device_id),
            ))?;

        device.healthy = false;

        // Trip circuit breaker — tracks failure trajectory independently of the health flag
        let mut breakers = self.circuit_breakers.write().await;
        let breaker = breakers.entry(device_id.to_string()).or_insert_with(CircuitBreaker::new);
        breaker.record_failure();

        tracing::warn!(
            "Device {} marked unhealthy (circuit failures: {}, state: {:?})",
            device_id, breaker.consecutive_failures, breaker.state
        );
        Ok(())
    }

    /// Record a successful allocation for a device, closing its circuit if half-open.
    pub async fn record_success(&self, device_id: &str) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(device_id) {
            if breaker.state == CircuitState::HalfOpen {
                tracing::info!("Device {} recovered — circuit breaker closed", device_id);
                breaker.record_success();
            }
        }
    }

    /// Get circuit breaker state for a device
    pub async fn circuit_state(&self, device_id: &str) -> Option<CircuitState> {
        let breakers = self.circuit_breakers.read().await;
        breakers.get(device_id).map(|b| b.state.clone())
    }
}

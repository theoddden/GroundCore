//! Physical transmission abstraction

use crate::command::CommandId;
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Transmission identifier
pub type TransmissionId = Uuid;
pub type TransmitterId = Uuid;

/// Transmitter status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransmitterStatus {
    Idle,
    Transmitting,
    Fault,
    Disabled,
}

/// Transmitter state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmitterState {
    pub status: TransmitterStatus,
    pub frequency: u64,
    pub power: f64,
    pub current_transmission: Option<TransmissionId>,
}

/// Transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transmission {
    pub id: TransmissionId,
    pub command_id: CommandId,
    pub data: Vec<u8>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Transmitter trait
#[async_trait::async_trait]
pub trait Transmitter: Send + Sync {
    /// Transmit data
    async fn transmit(&mut self, data: &[u8]) -> Result<TransmissionId>;

    /// Get transmitter status
    async fn get_status(&self) -> Result<TransmitterStatus>;

    /// Get transmitter state
    async fn get_state(&self) -> Result<TransmitterState>;

    /// Abort current transmission
    async fn abort(&mut self, id: TransmissionId) -> Result<()>;

    /// Set frequency
    async fn set_frequency(&mut self, frequency: u64) -> Result<()>;

    /// Set transmit power
    async fn set_power(&mut self, power: f64) -> Result<()>;

    /// Enable transmitter
    async fn enable(&mut self) -> Result<()>;

    /// Disable transmitter
    async fn disable(&mut self) -> Result<()>;
}

/// SDR-based transmitter implementation
pub struct SdrTransmitter {
    #[allow(dead_code)]
    id: TransmitterId,
    status: RwLock<TransmitterStatus>,
    frequency: AtomicU64,
    power: AtomicU64, // Stored as integer (power * 100 for 2 decimal places)
    current_transmission: RwLock<Option<TransmissionId>>,
    enabled: AtomicBool,
}

impl SdrTransmitter {
    /// Create a new SDR transmitter
    pub fn new(frequency: u64, power: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            status: RwLock::new(TransmitterStatus::Idle),
            frequency: AtomicU64::new(frequency),
            power: AtomicU64::new((power * 100.0) as u64),
            current_transmission: RwLock::new(None),
            enabled: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Transmitter for SdrTransmitter {
    async fn transmit(&mut self, data: &[u8]) -> Result<TransmissionId> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(GroundStationError::Hardware(
                "Transmitter disabled".to_string(),
            ));
        }

        let mut status = self.status.write().await;
        if *status != TransmitterStatus::Idle {
            return Err(GroundStationError::Hardware("Transmitter busy".to_string()));
        }

        *status = TransmitterStatus::Transmitting;
        drop(status);

        let transmission_id = Uuid::new_v4();
        let mut current = self.current_transmission.write().await;
        *current = Some(transmission_id);

        // In a real implementation, this would interface with SDR hardware
        // For now, simulate transmission
        tracing::info!("Transmitting {} bytes via SDR", data.len());

        Ok(transmission_id)
    }

    async fn get_status(&self) -> Result<TransmitterStatus> {
        let status = self.status.read().await;
        Ok(*status)
    }

    async fn get_state(&self) -> Result<TransmitterState> {
        let status = self.status.read().await;
        let frequency = self.frequency.load(Ordering::Relaxed);
        let power = self.power.load(Ordering::Relaxed) as f64 / 100.0;
        let current_transmission = self.current_transmission.read().await;

        Ok(TransmitterState {
            status: *status,
            frequency,
            power,
            current_transmission: *current_transmission,
        })
    }

    async fn abort(&mut self, id: TransmissionId) -> Result<()> {
        let current = self.current_transmission.read().await;
        if *current != Some(id) {
            return Err(GroundStationError::NotFound(format!("Transmission {}", id)));
        }

        let mut status = self.status.write().await;
        *status = TransmitterStatus::Idle;

        let mut current = self.current_transmission.write().await;
        *current = None;

        Ok(())
    }

    async fn set_frequency(&mut self, frequency: u64) -> Result<()> {
        self.frequency.store(frequency, Ordering::Relaxed);
        Ok(())
    }

    async fn set_power(&mut self, power: f64) -> Result<()> {
        if !(0.0..=1000.0).contains(&power) {
            return Err(GroundStationError::Validation(
                "Power out of range".to_string(),
            ));
        }
        self.power.store((power * 100.0) as u64, Ordering::Relaxed);
        Ok(())
    }

    async fn enable(&mut self) -> Result<()> {
        self.enabled.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn disable(&mut self) -> Result<()> {
        self.enabled.store(false, Ordering::Relaxed);
        Ok(())
    }
}

/// Satellite modem transmitter implementation (via serial/ethernet)
pub struct ModemTransmitter {
    #[allow(dead_code)]
    id: TransmitterId,
    status: RwLock<TransmitterStatus>,
    frequency: AtomicU64,
    power: AtomicU64,
    current_transmission: RwLock<Option<TransmissionId>>,
    enabled: AtomicBool,
}

impl ModemTransmitter {
    pub fn new(frequency: u64, power: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            status: RwLock::new(TransmitterStatus::Idle),
            frequency: AtomicU64::new(frequency),
            power: AtomicU64::new((power * 100.0) as u64),
            current_transmission: RwLock::new(None),
            enabled: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Transmitter for ModemTransmitter {
    async fn transmit(&mut self, data: &[u8]) -> Result<TransmissionId> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(GroundStationError::Hardware(
                "Transmitter disabled".to_string(),
            ));
        }

        let mut status = self.status.write().await;
        if *status != TransmitterStatus::Idle {
            return Err(GroundStationError::Hardware("Transmitter busy".to_string()));
        }

        *status = TransmitterStatus::Transmitting;
        drop(status);

        let transmission_id = Uuid::new_v4();
        let mut current = self.current_transmission.write().await;
        *current = Some(transmission_id);

        // In a real implementation, this would interface with satellite modem
        tracing::info!("Transmitting {} bytes via modem", data.len());

        Ok(transmission_id)
    }

    async fn get_status(&self) -> Result<TransmitterStatus> {
        let status = self.status.read().await;
        Ok(*status)
    }

    async fn get_state(&self) -> Result<TransmitterState> {
        let status = self.status.read().await;
        let frequency = self.frequency.load(Ordering::Relaxed);
        let power = self.power.load(Ordering::Relaxed) as f64 / 100.0;
        let current_transmission = self.current_transmission.read().await;

        Ok(TransmitterState {
            status: *status,
            frequency,
            power,
            current_transmission: *current_transmission,
        })
    }

    async fn abort(&mut self, id: TransmissionId) -> Result<()> {
        let current = self.current_transmission.read().await;
        if *current != Some(id) {
            return Err(GroundStationError::NotFound(format!("Transmission {}", id)));
        }

        let mut status = self.status.write().await;
        *status = TransmitterStatus::Idle;

        let mut current = self.current_transmission.write().await;
        *current = None;

        Ok(())
    }

    async fn set_frequency(&mut self, frequency: u64) -> Result<()> {
        self.frequency.store(frequency, Ordering::Relaxed);
        Ok(())
    }

    async fn set_power(&mut self, power: f64) -> Result<()> {
        if !(0.0..=1000.0).contains(&power) {
            return Err(GroundStationError::Validation(
                "Power out of range".to_string(),
            ));
        }
        self.power.store((power * 100.0) as u64, Ordering::Relaxed);
        Ok(())
    }

    async fn enable(&mut self) -> Result<()> {
        self.enabled.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn disable(&mut self) -> Result<()> {
        self.enabled.store(false, Ordering::Relaxed);
        Ok(())
    }
}

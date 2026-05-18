// Optical Terminal trait - vendor abstraction over SDA OCT
//
// The architectural insight: Mynaric and Tesat implement the same SDA OCT
// Standard at the optical layer. They differ at the management interface.
// This trait abstracts the management interface, not the optical layer.

use crate::{
    TerminalId, SerialNumber, OctConfiguration, OctStandardVersion,
    topology::link_state::LinkPhase,
};
use crate::physical::vendors::Vendor;
use crate::physical::terminal::ResetLevel;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::pin::Pin;
use tokio::sync::mpsc;

/// Terminal capability
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TerminalCapability {
    pub max_data_rate: crate::DataRate,
    pub max_pointing_accuracy_rad: f64,
    pub supported_standards: Vec<OctStandardVersion>,
    pub beam_divergence_mrad: f64,
    pub max_range_km: f64,
}

/// Calibration report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalibrationReport {
    pub calibrated_at: DateTime<Utc>,
    pub pointing_accuracy_rad: f64,
    pub alignment_error_rad: f64,
    pub next_calibration_due: DateTime<Utc>,
}

/// Acquisition schedule for PAT
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AcquisitionSchedule {
    pub acquisition_id: crate::AcquisitionId,
    pub target_t0: DateTime<Utc>,
    pub peer_terminal: TerminalId,
    pub initial_pointing: (f64, f64, f64), // azimuth, elevation, range
}

/// PAT handle for ongoing acquisition
#[derive(Debug, Clone)]
pub struct PatHandle {
    pub acquisition_id: crate::AcquisitionId,
    pub started_at: DateTime<Utc>,
}

/// Ethernet endpoint for data path
#[derive(Debug, Clone)]
pub struct EthernetEndpoint {
    pub ip: String,
    pub port: u16,
    pub vlan: Option<u16>,
}

/// Telemetry stream
pub type TelemetryStream = Pin<Box<dyn futures::Stream<Item = TelemetryFrame> + Send>>;

/// Telemetry frame
#[derive(Debug, Clone)]
pub struct TelemetryFrame {
    pub timestamp: DateTime<Utc>,
    pub data: Vec<u8>,
}

/// Health report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthReport {
    pub overall_health: crate::HealthMetrics,
    pub component_health: Vec<(String, f64)>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub reported_at: DateTime<Utc>,
}

/// Optical terminal trait - vendor abstraction
#[async_trait]
pub trait OpticalTerminal: Send + Sync {
    // Identity
    fn vendor(&self) -> Vendor;
    fn model(&self) -> &str;
    fn serial(&self) -> SerialNumber;

    // Standard compliance
    fn oct_standard_versions(&self) -> Vec<OctStandardVersion>;
    fn capabilities(&self) -> TerminalCapability;

    // Configuration
    async fn configure(&mut self, config: OctConfiguration) -> Result<(), TerminalError>;
    async fn calibrate(&mut self) -> Result<CalibrationReport, TerminalError>;

    // PAT
    async fn schedule_acquisition(&mut self, schedule: AcquisitionSchedule) -> Result<(), TerminalError>;
    async fn begin_pat(&mut self) -> Result<PatHandle, TerminalError>;
    async fn cancel_pat(&mut self, acquisition_id: crate::AcquisitionId) -> Result<(), TerminalError>;

    // Operations
    async fn data_path(&self) -> Result<EthernetEndpoint, TerminalError>;
    async fn telemetry_stream(&self) -> Result<TelemetryStream, TerminalError>;
    async fn health_check(&self) -> Result<HealthReport, TerminalError>;

    // Recovery
    async fn reset(&mut self, level: ResetLevel) -> Result<(), TerminalError>;
    async fn safe_mode(&mut self) -> Result<(), TerminalError>;

    // Status
    async fn get_status(&self) -> Result<TerminalStatus, TerminalError>;
}

/// Terminal status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TerminalStatus {
    pub operational: bool,
    pub current_phase: LinkPhase,
    pub current_link: Option<crate::LinkId>,
    pub temperature_c: f64,
    pub power_watts: f64,
    pub last_updated: DateTime<Utc>,
}

/// Terminal error
#[derive(Debug, Clone, thiserror::Error)]
pub enum TerminalError {
    #[error("Communication error: {0}")]
    CommunicationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Hardware error: {0}")]
    HardwareError(String),

    #[error("PAT error: {0}")]
    PatError(String),

    #[error("Not supported by this terminal: {0}")]
    NotSupported(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

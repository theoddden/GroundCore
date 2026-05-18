// Optical Terminal trait - vendor abstraction over SDA OCT

use crate::oct::OctConfiguration;
use crate::pat::{AcquisitionPlan, AcquisitionResult};
use crate::terminal::telemetry::ResetLevel;
use crate::terminal::{HealthReport, TelemetryStream};
use async_trait::async_trait;
use chrono::DateTime;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("Communication error: {0}")]
    CommunicationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Calibration error: {0}")]
    CalibrationError(String),

    #[error("Acquisition error: {0}")]
    AcquisitionError(String),

    #[error("Hardware error: {0}")]
    HardwareError(String),

    #[error("Not ready: {0}")]
    NotReady(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Terminal status
#[derive(Debug, Clone)]
pub struct TerminalStatus {
    pub operational: bool,
    pub current_phase: String,
    pub current_link: Option<String>,
    pub temperature_c: f64,
    pub power_watts: f64,
    pub last_updated: DateTime<chrono::Utc>,
}

/// Calibration report
#[derive(Debug, Clone)]
pub struct CalibrationReport {
    pub calibrated_at: DateTime<chrono::Utc>,
    pub pointing_offset_urad: (f64, f64),
    pub power_calibration_db: f64,
    pub success: bool,
}

/// Ethernet endpoint for data
#[derive(Debug, Clone)]
pub struct EthernetEndpoint {
    pub ip_address: String,
    pub port: u16,
    pub mtu: u16,
}

/// Optical Terminal trait
#[async_trait]
pub trait OpticalTerminal: Send + Sync {
    // Identity
    fn vendor(&self) -> String;
    fn model(&self) -> &str;
    fn serial_number(&self) -> &str;
    fn capabilities(&self) -> &crate::terminal::TerminalCapability;

    // Configuration
    async fn configure(&mut self, config: OctConfiguration) -> Result<(), TerminalError>;
    async fn calibrate(&mut self) -> Result<CalibrationReport, TerminalError>;

    // PAT operations
    async fn schedule_acquisition(&mut self, plan: AcquisitionPlan) -> Result<(), TerminalError>;
    async fn execute_acquisition(&mut self) -> Result<AcquisitionResult, TerminalError>;
    async fn start_tracking(&mut self) -> Result<Box<dyn TrackingHandle>, TerminalError>;

    // Operations
    async fn data_endpoint(&self) -> Result<EthernetEndpoint, TerminalError>;
    async fn telemetry_stream(&self) -> Result<Box<dyn TelemetryStream>, TerminalError>;
    async fn health(&self) -> Result<HealthReport, TerminalError>;
    async fn status(&self) -> Result<TerminalStatus, TerminalError>;

    // Recovery
    async fn reset(&mut self, level: ResetLevel) -> Result<(), TerminalError>;
    async fn enter_safe_mode(&mut self) -> Result<(), TerminalError>;
}

/// Tracking handle for active tracking
#[async_trait]
pub trait TrackingHandle: Send + Sync {
    async fn get_metrics(&self) -> Result<TrackingMetrics, TerminalError>;
    async fn adjust_pointing(
        &mut self,
        azimuth_urad: f64,
        elevation_urad: f64,
    ) -> Result<(), TerminalError>;
    async fn stop(&mut self) -> Result<(), TerminalError>;
}

/// Tracking metrics
#[derive(Debug, Clone)]
pub struct TrackingMetrics {
    pub pointing_error_urad: f64,
    pub signal_to_noise_db: f64,
    pub lock_confidence: f64,
    pub data_rate_actual: u64,
    pub bit_error_rate: f64,
}

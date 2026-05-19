//! Central error types for the ground station system

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GroundStationError {
    #[error("RF processing error: {0}")]
    RfProcessing(String),

    #[error("Hardware error: {0}")]
    Hardware(String),

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Satellite tracking error: {0}")]
    Tracking(String),

    #[error("Federation error: {0}")]
    Federation(String),

    #[error("Regulatory violation: {0}")]
    Regulatory(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("License not found for band: {0}")]
    LicenseNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, GroundStationError>;

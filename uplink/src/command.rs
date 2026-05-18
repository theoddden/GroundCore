//! Command representation and validation

use chrono::{DateTime, Utc};
use ground_core::{GroundStationError, Result, SatelliteId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Command identifier
pub type CommandId = Uuid;

/// Command priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CommandPriority {
    Emergency = 3,
    High = 2,
    Normal = 1,
    Low = 0,
}

/// Command execution timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    /// Execute immediately
    Immediate,
    /// Execute at specific time
    TimeTagged(DateTime<Utc>),
    /// Execute if condition is met
    Conditional(String),
}

/// Command state in the lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandState {
    Queued,
    Scheduled,
    Transmitting,
    AwaitingAck,
    Acknowledged,
    Failed,
    Timeout,
    Cancelled,
}

/// Satellite command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Unique command identifier
    pub id: CommandId,
    /// Target satellite
    pub satellite_id: SatelliteId,
    /// Command payload (encrypted after validation)
    pub payload: Vec<u8>,
    /// Command type (immediate, time-tagged, conditional)
    pub command_type: CommandType,
    /// Command priority
    pub priority: CommandPriority,
    /// Authentication token
    pub auth_token: Vec<u8>,
    /// Command timestamp
    pub created_at: DateTime<Utc>,
    /// Command version (for protocol versioning)
    pub version: u32,
    /// Required capabilities for satellite
    pub required_capabilities: Vec<String>,
}

impl Command {
    /// Create a new command
    pub fn new(
        satellite_id: SatelliteId,
        payload: Vec<u8>,
        command_type: CommandType,
        priority: CommandPriority,
        auth_token: Vec<u8>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            satellite_id,
            payload,
            command_type,
            priority,
            auth_token,
            created_at: Utc::now(),
            version: 1,
            required_capabilities: Vec::new(),
        }
    }

    /// Validate the command
    pub fn validate(&self, satellite_capabilities: &[String]) -> Result<()> {
        // Check that satellite has required capabilities
        for required in &self.required_capabilities {
            if !satellite_capabilities.contains(required) {
                return Err(GroundStationError::Validation(format!(
                    "Satellite lacks required capability: {}",
                    required
                )));
            }
        }

        // Validate payload size
        if self.payload.is_empty() {
            return Err(GroundStationError::Validation(
                "Command payload cannot be empty".to_string(),
            ));
        }

        if self.payload.len() > 65536 {
            return Err(GroundStationError::Validation(
                "Command payload too large (max 64KB)".to_string(),
            ));
        }

        // Validate authentication token
        if self.auth_token.is_empty() {
            return Err(GroundStationError::Validation(
                "Command requires authentication token".to_string(),
            ));
        }

        Ok(())
    }

    /// Get execution time for time-tagged commands
    pub fn execution_time(&self) -> Option<DateTime<Utc>> {
        match &self.command_type {
            CommandType::TimeTagged(time) => Some(*time),
            _ => None,
        }
    }

    /// Check if command should execute now
    pub fn should_execute(&self) -> bool {
        match &self.command_type {
            CommandType::Immediate => true,
            CommandType::TimeTagged(time) => *time <= Utc::now(),
            CommandType::Conditional(_) => false, // Evaluated separately
        }
    }
}

/// Command validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

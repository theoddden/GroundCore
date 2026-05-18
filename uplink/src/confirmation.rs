//! Confirmation tracking with ACK/NACK and retry logic

use crate::command::{Command, CommandId, CommandState};
use chrono::{DateTime, Duration, Utc};
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration as StdDuration;
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Command outcome
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandOutcome {
    /// Command acknowledged by satellite
    Acknowledged,
    /// Command rejected by satellite (NACK)
    Rejected(String),
    /// Command timed out (no response)
    Timeout,
    /// Transmission failed
    TransmissionFailed,
}

/// Confirmation tracking entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationEntry {
    /// Command ID
    pub command_id: CommandId,
    /// Transmission ID
    pub transmission_id: crate::transmitter::TransmissionId,
    /// Command state
    pub state: CommandState,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Maximum retries
    pub max_retries: u32,
    /// Transmission timestamp
    pub transmitted_at: DateTime<Utc>,
    /// Expected ACK timeout
    pub timeout: Duration,
    /// Last update timestamp
    pub last_update: DateTime<Utc>,
}

/// Confirmation tracker
pub struct ConfirmationTracker {
    /// Tracking entries (command_id -> entry)
    entries: RwLock<HashMap<CommandId, ConfirmationEntry>>,
    /// Retry backoff base (exponential backoff)
    backoff_base: StdDuration,
    /// Default timeout
    default_timeout: Duration,
}

impl ConfirmationTracker {
    /// Create a new confirmation tracker
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            backoff_base: StdDuration::from_secs(1),
            default_timeout,
        }
    }

    /// Track a command transmission
    pub async fn track(
        &self,
        command: &Command,
        transmission_id: crate::transmitter::TransmissionId,
    ) -> Result<()> {
        let entry = ConfirmationEntry {
            command_id: command.id,
            transmission_id,
            state: CommandState::AwaitingAck,
            retry_count: 0,
            max_retries: 3,
            transmitted_at: Utc::now(),
            timeout: self.default_timeout,
            last_update: Utc::now(),
        };

        let mut entries = self.entries.write().await;
        entries.insert(command.id, entry);
        Ok(())
    }

    /// Handle ACK from satellite
    pub async fn handle_ack(&self, command_id: CommandId) -> Result<CommandOutcome> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&command_id) {
            entry.state = CommandState::Acknowledged;
            entry.last_update = Utc::now();
            Ok(CommandOutcome::Acknowledged)
        } else {
            Err(GroundStationError::NotFound(format!(
                "Command {}",
                command_id
            )))
        }
    }

    /// Handle NACK from satellite
    pub async fn handle_nack(
        &self,
        command_id: CommandId,
        reason: String,
    ) -> Result<CommandOutcome> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&command_id) {
            entry.state = CommandState::Failed;
            entry.last_update = Utc::now();
            Ok(CommandOutcome::Rejected(reason))
        } else {
            Err(GroundStationError::NotFound(format!(
                "Command {}",
                command_id
            )))
        }
    }

    /// Check for timed out commands
    pub async fn check_timeouts(&self) -> Vec<CommandId> {
        let mut entries = self.entries.write().await;
        let mut timed_out = Vec::new();
        let now = Utc::now();

        for (command_id, entry) in entries.iter_mut() {
            if entry.state == CommandState::AwaitingAck {
                let elapsed = now - entry.transmitted_at;
                if elapsed > entry.timeout {
                    entry.state = CommandState::Timeout;
                    entry.last_update = now;
                    timed_out.push(*command_id);
                }
            }
        }

        timed_out
    }

    /// Calculate retry delay with exponential backoff
    fn retry_delay(&self, retry_count: u32) -> StdDuration {
        let delay_ms = (2_u64.pow(retry_count.min(6)) * 1000) as u64;
        StdDuration::from_millis(delay_ms)
    }

    /// Retry a failed command
    pub async fn retry(&self, command_id: CommandId) -> Result<bool> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&command_id) {
            if entry.retry_count >= entry.max_retries {
                entry.state = CommandState::Failed;
                return Ok(false); // Max retries exceeded
            }

            entry.retry_count += 1;
            entry.state = CommandState::Transmitting;
            entry.transmitted_at = Utc::now();
            entry.last_update = Utc::now();
            Ok(true)
        } else {
            Err(GroundStationError::NotFound(format!(
                "Command {}",
                command_id
            )))
        }
    }

    /// Get entry for a command
    pub async fn get(&self, command_id: CommandId) -> Option<ConfirmationEntry> {
        let entries = self.entries.read().await;
        entries.get(&command_id).cloned()
    }

    /// Remove entry for a command
    pub async fn remove(&self, command_id: CommandId) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.remove(&command_id);
        Ok(())
    }

    /// Get all pending commands (awaiting ACK)
    pub async fn pending(&self) -> Vec<ConfirmationEntry> {
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|e| e.state == CommandState::AwaitingAck)
            .cloned()
            .collect()
    }

    /// Get all failed commands
    pub async fn failed(&self) -> Vec<ConfirmationEntry> {
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|e| matches!(e.state, CommandState::Failed | CommandState::Timeout))
            .cloned()
            .collect()
    }

    /// Get statistics
    pub async fn stats(&self) -> ConfirmationStats {
        let entries = self.entries.read().await;
        let total = entries.len();
        let pending = entries
            .values()
            .filter(|e| e.state == CommandState::AwaitingAck)
            .count();
        let acknowledged = entries
            .values()
            .filter(|e| e.state == CommandState::Acknowledged)
            .count();
        let failed = entries
            .values()
            .filter(|e| matches!(e.state, CommandState::Failed | CommandState::Timeout))
            .count();
        let transmitting = entries
            .values()
            .filter(|e| e.state == CommandState::Transmitting)
            .count();

        ConfirmationStats {
            total,
            pending,
            acknowledged,
            failed,
            transmitting,
        }
    }
}

/// Confirmation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationStats {
    pub total: usize,
    pub pending: usize,
    pub acknowledged: usize,
    pub failed: usize,
    pub transmitting: usize,
}

/// Background task to check timeouts and retry failed commands
pub async fn timeout_checker_task(tracker: ConfirmationTracker, interval: StdDuration) {
    loop {
        sleep(interval).await;

        // Check for timeouts
        let timed_out = tracker.check_timeouts().await;

        // Retry timed out commands
        for command_id in timed_out {
            if let Ok(can_retry) = tracker.retry(command_id).await {
                if can_retry {
                    tracing::info!("Retrying command {} after timeout", command_id);
                } else {
                    tracing::warn!("Command {} exceeded max retries", command_id);
                }
            }
        }
    }
}

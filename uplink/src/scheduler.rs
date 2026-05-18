//! Transmission scheduling with pass window integration

use crate::command::{Command, CommandId, CommandPriority};
use chrono::{DateTime, Duration, Utc};
use ground_core::{PassId, Result, GroundStationError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;

/// Transmission window (when satellite is visible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmissionWindow {
    /// Pass identifier
    pub pass_id: PassId,
    /// Window start
    pub start: DateTime<Utc>,
    /// Window end
    pub end: DateTime<Utc>,
    /// Maximum uplink rate (commands per second)
    pub max_rate: f64,
    /// Power budget (watts)
    pub power_budget: f64,
}

impl TransmissionWindow {
    /// Check if time is within window
    pub fn contains(&self, time: DateTime<Utc>) -> bool {
        time >= self.start && time <= self.end
    }

    /// Window duration
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }

    /// Time until window starts
    pub fn time_until_start(&self) -> Option<Duration> {
        let now = Utc::now();
        if self.start > now {
            Some(self.start - now)
        } else {
            None
        }
    }
}

/// Scheduled transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTransmission {
    /// Command ID
    pub command_id: CommandId,
    /// Transmission window
    pub window: TransmissionWindow,
    /// Scheduled transmission time
    pub transmission_time: DateTime<Utc>,
    /// Estimated duration
    pub estimated_duration: Duration,
    /// Doppler compensation (Hz)
    pub doppler_compensation: f64,
}

/// Transmission scheduler
pub struct TransmissionScheduler {
    /// Transmission windows (pass_id -> window)
    windows: RwLock<HashMap<PassId, TransmissionWindow>>,
    /// Scheduled transmissions (command_id -> transmission)
    scheduled: RwLock<HashMap<CommandId, ScheduledTransmission>>,
    /// Pending commands (waiting for window)
    pending: RwLock<VecDeque<Command>>,
    /// Conflict resolution strategy
    conflict_strategy: ConflictStrategy,
}

/// Conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Priority-based (higher priority wins)
    Priority,
    /// First-come-first-served
    Fcfs,
    /// Reject conflicts
    Reject,
}

impl TransmissionScheduler {
    /// Create a new scheduler
    pub fn new(conflict_strategy: ConflictStrategy) -> Self {
        Self {
            windows: RwLock::new(HashMap::new()),
            scheduled: RwLock::new(HashMap::new()),
            pending: RwLock::new(VecDeque::new()),
            conflict_strategy,
        }
    }

    /// Add a transmission window
    pub async fn add_window(&self, window: TransmissionWindow) -> Result<()> {
        let mut windows = self.windows.write().await;
        windows.insert(window.pass_id.clone(), window);
        Ok(())
    }

    /// Remove a transmission window
    pub async fn remove_window(&self, pass_id: &PassId) -> Result<()> {
        let mut windows = self.windows.write().await;
        windows.remove(pass_id);
        Ok(())
    }

    /// Get a transmission window
    pub async fn get_window(&self, pass_id: &PassId) -> Option<TransmissionWindow> {
        let windows = self.windows.read().await;
        windows.get(pass_id).cloned()
    }

    /// Get active windows (windows that contain current time)
    pub async fn active_windows(&self) -> Vec<TransmissionWindow> {
        let windows = self.windows.read().await;
        let now = Utc::now();
        windows.values()
            .filter(|w| w.contains(now))
            .cloned()
            .collect()
    }

    /// Schedule a command for transmission
    pub async fn schedule(&self, command: Command) -> Result<ScheduledTransmission> {
        let now = Utc::now();
        
        // Find suitable transmission window
        let windows = self.windows.read().await;
        let window = windows.values()
            .find(|w| w.contains(now) || w.time_until_start().is_some())
            .ok_or_else(|| GroundStationError::Validation("No suitable transmission window".to_string()))?;
        
        // Calculate transmission time
        let transmission_time = if window.contains(now) {
            now
        } else {
            window.start
        };
        
        // Estimate duration (based on command size and max rate)
        let estimated_duration = Duration::milliseconds(
            ((command.payload.len() as f64 / window.max_rate) * 1000.0) as i64
        );
        
        // Doppler compensation placeholder (would use tracking predictions)
        let doppler_compensation = 0.0;
        
        let scheduled = ScheduledTransmission {
            command_id: command.id,
            window: window.clone(),
            transmission_time,
            estimated_duration,
            doppler_compensation,
        };
        
        // Check for conflicts
        if self.has_conflict(&scheduled).await? {
            match self.conflict_strategy {
                ConflictStrategy::Priority => {
                    // Check if this command has higher priority than conflicting ones
                    if !self.can_override(&command).await {
                        return Err(GroundStationError::Validation(
                            "Transmission conflict - lower priority".to_string()
                        ));
                    }
                }
                ConflictStrategy::Fcfs => {
                    return Err(GroundStationError::Validation(
                        "Transmission conflict - first come first served".to_string()
                    ));
                }
                ConflictStrategy::Reject => {
                    return Err(GroundStationError::Validation(
                        "Transmission conflict - rejected".to_string()
                    ));
                }
            }
        }
        
        // Store scheduled transmission
        let mut scheduled_map = self.scheduled.write().await;
        scheduled_map.insert(command.id, scheduled.clone());
        
        Ok(scheduled)
    }

    /// Check if transmission conflicts with existing schedule
    async fn has_conflict(&self, scheduled: &ScheduledTransmission) -> Result<bool> {
        let scheduled_map = self.scheduled.read().await;
        
        for existing in scheduled_map.values() {
            // Check time overlap
            let overlap = scheduled.transmission_time < existing.transmission_time + existing.estimated_duration
                && existing.transmission_time < scheduled.transmission_time + scheduled.estimated_duration;
            
            if overlap {
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    /// Check if command can override existing schedule (higher priority)
    async fn can_override(&self, command: &Command) -> bool {
        let scheduled_map = self.scheduled.read().await;
        
        if let Some(existing) = scheduled_map.get(&command.id) {
            return false; // Can't override self
        }
        
        // Check if all conflicting commands have lower priority
        for existing in scheduled_map.values() {
            if let Some(existing_cmd) = self.pending.read().await.iter().find(|c| c.id == existing.command_id) {
                if existing_cmd.priority >= command.priority {
                    return false;
                }
            }
        }
        
        true
    }

    /// Get scheduled transmission for a command
    pub async fn get_scheduled(&self, command_id: CommandId) -> Option<ScheduledTransmission> {
        let scheduled = self.scheduled.read().await;
        scheduled.get(&command_id).cloned()
    }

    /// Remove scheduled transmission
    pub async fn remove_scheduled(&self, command_id: CommandId) -> Result<()> {
        let mut scheduled = self.scheduled.write().await;
        scheduled.remove(&command_id);
        Ok(())
    }

    /// Get all scheduled transmissions
    pub async fn all_scheduled(&self) -> Vec<ScheduledTransmission> {
        let scheduled = self.scheduled.read().await;
        scheduled.values().cloned().collect()
    }
}

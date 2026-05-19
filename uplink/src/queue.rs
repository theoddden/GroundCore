//! Command queuing with priority and persistence

use crate::command::{Command, CommandId, CommandState};
use chrono::{DateTime, Utc};
use ground_core::{GroundStationError, Result};
use priority_queue::PriorityQueue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Queued command with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueuedCommand {
    /// The command
    pub command: Command,
    /// Current state
    pub state: CommandState,
    /// When it was queued
    pub queued_at: DateTime<Utc>,
    /// Scheduled transmission time (if applicable)
    pub scheduled_at: Option<DateTime<Utc>>,
    /// Retry count
    pub retry_count: u32,
    /// Max retries
    pub max_retries: u32,
}

impl QueuedCommand {
    pub fn new(command: Command) -> Self {
        Self {
            command,
            state: CommandState::Queued,
            queued_at: Utc::now(),
            scheduled_at: None,
            retry_count: 0,
            max_retries: 3,
        }
    }
}

/// Command queue with priority
pub struct CommandQueue {
    /// Priority queue for commands
    queue: RwLock<PriorityQueue<CommandId, QueuedCommand>>,
    /// Command lookup by ID
    commands: RwLock<HashMap<CommandId, QueuedCommand>>,
    /// Dependencies (command_id -> depends_on_command_ids)
    dependencies: RwLock<HashMap<CommandId, Vec<CommandId>>>,
}

impl CommandQueue {
    /// Create a new command queue
    pub fn new() -> Self {
        Self {
            queue: RwLock::new(PriorityQueue::new()),
            commands: RwLock::new(HashMap::new()),
            dependencies: RwLock::new(HashMap::new()),
        }
    }

    /// Add a command to the queue
    pub async fn enqueue(&self, command: Command) -> Result<CommandId> {
        let id = command.id;
        let queued = QueuedCommand::new(command);

        let mut queue = self.queue.write().await;
        let mut commands = self.commands.write().await;

        queue.push(id, queued.clone());
        commands.insert(id, queued);

        Ok(id)
    }

    /// Add a command with dependencies
    pub async fn enqueue_with_dependencies(
        &self,
        command: Command,
        depends_on: Vec<CommandId>,
    ) -> Result<CommandId> {
        let id = command.id;
        let queued = QueuedCommand::new(command);

        let mut queue = self.queue.write().await;
        let mut commands = self.commands.write().await;
        let mut dependencies = self.dependencies.write().await;

        queue.push(id, queued.clone());
        commands.insert(id, queued);
        dependencies.insert(id, depends_on);

        Ok(id)
    }

    /// Get the next command to transmit (highest priority, dependencies satisfied)
    pub async fn dequeue(&self) -> Option<Command> {
        let mut queue = self.queue.write().await;
        let commands = self.commands.read().await;
        let dependencies = self.dependencies.read().await;

        // Find the highest priority command with satisfied dependencies
        while let Some((id, queued)) = queue.pop() {
            // Check dependencies
            if let Some(deps) = dependencies.get(&id) {
                let all_deps_satisfied = deps.iter().all(|dep_id| {
                    commands
                        .get(dep_id)
                        .map(|c| c.state == CommandState::Acknowledged)
                        .unwrap_or(false)
                });

                if !all_deps_satisfied {
                    // Put it back and continue
                    queue.push(id, queued);
                    continue;
                }
            }

            // Update state
            let mut commands_mut = self.commands.write().await;
            if let Some(cmd) = commands_mut.get_mut(&id) {
                cmd.state = CommandState::Scheduled;
                return Some(cmd.command.clone());
            }
        }

        None
    }

    /// Get a command by ID
    pub async fn get(&self, id: CommandId) -> Option<QueuedCommand> {
        let commands = self.commands.read().await;
        commands.get(&id).cloned()
    }

    /// Update command state
    pub async fn update_state(&self, id: CommandId, state: CommandState) -> Result<()> {
        let mut commands = self.commands.write().await;
        if let Some(queued) = commands.get_mut(&id) {
            queued.state = state;
            Ok(())
        } else {
            Err(GroundStationError::NotFound(format!("Command {}", id)))
        }
    }

    /// Increment retry count
    pub async fn increment_retry(&self, id: CommandId) -> Result<()> {
        let mut commands = self.commands.write().await;
        if let Some(queued) = commands.get_mut(&id) {
            queued.retry_count += 1;
            Ok(())
        } else {
            Err(GroundStationError::NotFound(format!("Command {}", id)))
        }
    }

    /// Cancel a command
    pub async fn cancel(&self, id: CommandId) -> Result<()> {
        let mut commands = self.commands.write().await;
        if let Some(queued) = commands.get_mut(&id) {
            queued.state = CommandState::Cancelled;
            Ok(())
        } else {
            Err(GroundStationError::NotFound(format!("Command {}", id)))
        }
    }

    /// Get queue statistics
    pub async fn stats(&self) -> QueueStats {
        let commands = self.commands.read().await;
        let total = commands.len();
        let queued = commands
            .values()
            .filter(|c| c.state == CommandState::Queued)
            .count();
        let scheduled = commands
            .values()
            .filter(|c| c.state == CommandState::Scheduled)
            .count();
        let transmitting = commands
            .values()
            .filter(|c| c.state == CommandState::Transmitting)
            .count();
        let awaiting = commands
            .values()
            .filter(|c| c.state == CommandState::AwaitingAck)
            .count();
        let acknowledged = commands
            .values()
            .filter(|c| c.state == CommandState::Acknowledged)
            .count();
        let failed = commands
            .values()
            .filter(|c| c.state == CommandState::Failed)
            .count();
        let timeout = commands
            .values()
            .filter(|c| c.state == CommandState::Timeout)
            .count();

        QueueStats {
            total,
            queued,
            scheduled,
            transmitting,
            awaiting,
            acknowledged,
            failed,
            timeout,
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub total: usize,
    pub queued: usize,
    pub scheduled: usize,
    pub transmitting: usize,
    pub awaiting: usize,
    pub acknowledged: usize,
    pub failed: usize,
    pub timeout: usize,
}

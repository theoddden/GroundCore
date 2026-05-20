// Tasking Scheduler - places tasks on timeline considering dependencies and constraints

use crate::mission::{LinkReservation, SatelliteTask, TaskType};
use crate::{Priority, SatelliteId, TaskId};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Tasking scheduler
pub struct TaskingScheduler {
    task_queue: VecDeque<SatelliteTask>,
    scheduled_tasks: HashMap<TaskId, ScheduledTask>,
    resource_conflicts: Vec<ConflictResolution>,
}

/// Scheduled task with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub task: SatelliteTask,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub dependencies_satisfied: bool,
}

/// Conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolution {
    pub conflict_type: ConflictType,
    pub resolution: ResolutionStrategy,
    pub affected_tasks: Vec<TaskId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    TerminalContention {
        terminal_id: crate::TerminalId,
        tasks: Vec<TaskId>,
    },
    TimingOverlap {
        tasks: Vec<TaskId>,
    },
    ResourceExhaustion {
        satellite_id: SatelliteId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    PrioritizeBySLA,
    DelayLowerPriority,
    SplitTask,
    RejectTask,
}

impl TaskingScheduler {
    pub fn new() -> Self {
        Self {
            task_queue: VecDeque::new(),
            scheduled_tasks: HashMap::new(),
            resource_conflicts: Vec::new(),
        }
    }

    /// Add task to scheduling queue
    pub fn enqueue(&mut self, task: SatelliteTask) {
        self.task_queue.push_back(task);
    }

    /// Schedule all queued tasks
    pub fn schedule(&mut self) -> Result<(), ScheduleError> {
        while let Some(task) = self.task_queue.pop_front() {
            self.schedule_single_task(task)?;
        }
        Ok(())
    }

    fn schedule_single_task(&mut self, task: SatelliteTask) -> Result<(), ScheduleError> {
        // Check if dependencies are satisfied
        let deps_satisfied = task
            .dependencies
            .iter()
            .all(|dep_id| self.scheduled_tasks.contains_key(dep_id));

        if !deps_satisfied {
            // Re-queue task for later
            self.task_queue.push_back(task);
            return Ok(());
        }

        // Calculate timing based on task type
        let (start, end) = self.calculate_task_timing(&task)?;

        let scheduled = ScheduledTask {
            task,
            scheduled_start: start,
            scheduled_end: end,
            dependencies_satisfied: true,
        };

        // Check for conflicts
        if let Some(conflict) = self.detect_conflicts(&scheduled) {
            self.resolve_conflict(conflict)?;
        }

        self.scheduled_tasks
            .insert(scheduled.task.task_id, scheduled);
        Ok(())
    }

    fn calculate_task_timing(
        &self,
        task: &SatelliteTask,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), ScheduleError> {
        let window = &task.scheduled_window;

        // Default task duration based on type
        let duration = match &task.task_type {
            TaskType::OpticalLinkEstablishment { .. } => Duration::seconds(30), // PAT + establishment
            TaskType::DataTransfer { volume, .. } => {
                // Estimate based on volume and assumed bandwidth
                let bytes = volume.0 as f64;
                let bandwidth = 1e9; // 1 Gbps assumption
                let seconds = bytes / (bandwidth / 8.0);
                Duration::seconds(seconds as i64)
            }
            TaskType::Observation { .. } => Duration::minutes(5),
            TaskType::Downlink { .. } => Duration::minutes(10),
        };

        // Schedule within window, accounting for dependencies
        let start = if task.dependencies.is_empty() {
            window.start
        } else {
            // Start after latest dependency
            let latest_dep = task
                .dependencies
                .iter()
                .filter_map(|dep_id| self.scheduled_tasks.get(dep_id))
                .map(|st| st.scheduled_end)
                .max()
                .unwrap_or(window.start);

            latest_dep.max(window.start)
        };

        let end = start + duration;

        if end > window.end {
            return Err(ScheduleError::WindowExceeded {
                task_id: task.task_id,
                required: duration,
                available: window.end - start,
            });
        }

        Ok((start, end))
    }

    fn detect_conflicts(&self, scheduled: &ScheduledTask) -> Option<ConflictResolution> {
        // Check for terminal contention
        if let TaskType::OpticalLinkEstablishment { peer_terminal, .. } = &scheduled.task.task_type
        {
            for existing in self.scheduled_tasks.values() {
                if let TaskType::OpticalLinkEstablishment {
                    peer_terminal: existing_peer,
                    ..
                } = &existing.task.task_type
                {
                    if peer_terminal == existing_peer
                        && scheduled
                            .task
                            .scheduled_window
                            .overlaps(&existing.task.scheduled_window)
                    {
                        return Some(ConflictResolution {
                            conflict_type: ConflictType::TerminalContention {
                                terminal_id: peer_terminal.clone(),
                                tasks: vec![scheduled.task.task_id, existing.task.task_id],
                            },
                            resolution: ResolutionStrategy::PrioritizeBySLA,
                            affected_tasks: vec![scheduled.task.task_id, existing.task.task_id],
                        });
                    }
                }
            }
        }

        None
    }

    fn resolve_conflict(&mut self, conflict: ConflictResolution) -> Result<(), ScheduleError> {
        self.resource_conflicts.push(conflict);
        Ok(())
    }

    /// Get scheduled tasks for a satellite
    pub fn get_satellite_tasks(&self, satellite_id: &SatelliteId) -> Vec<&ScheduledTask> {
        self.scheduled_tasks
            .values()
            .filter(|st| st.task.satellite_id == *satellite_id)
            .collect()
    }

    /// Get all scheduled tasks
    pub fn get_all_tasks(&self) -> Vec<&ScheduledTask> {
        self.scheduled_tasks.values().collect()
    }
}

impl Default for TaskingScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Schedule error
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScheduleError {
    #[error("Task {task_id} cannot fit in window: required {required:?}, available {available:?}")]
    WindowExceeded {
        task_id: TaskId,
        required: Duration,
        available: Duration,
    },

    #[error("Dependency not satisfied for task {0}")]
    DependencyNotSatisfied(TaskId),

    #[error("Resource conflict: {0}")]
    ResourceConflict(String),
}

//! Pass subprocess management

use crate::version::Version;
use chrono::{DateTime, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Process state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    Starting,
    Running,
    Completing,
    Completed,
    Failed,
    Killed,
}

/// Process handle for a pass subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessHandle {
    /// Process ID
    pub pid: u32,
    /// Binary version
    pub binary_version: Version,
    /// When this process was started
    pub started_at: DateTime<Utc>,
    /// Current state
    pub state: ProcessState,
}

/// Pass subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassProcess {
    /// Pass ID
    pub pass_id: PassId,
    /// Process handle
    pub process: ProcessHandle,
    /// Allocated hardware
    pub allocated_hardware: Vec<String>,
    /// Shared memory handle for state
    pub state_handle: SharedMemoryHandle,
}

/// Shared memory handle for inter-process communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryHandle {
    /// Memory region ID
    pub region_id: String,
    /// Size in bytes
    pub size: usize,
    /// Schema version
    pub schema_version: String,
}

impl PassProcess {
    pub fn new(
        pass_id: PassId,
        pid: u32,
        binary_version: Version,
        allocated_hardware: Vec<String>,
        state_handle: SharedMemoryHandle,
    ) -> Self {
        Self {
            pass_id,
            process: ProcessHandle {
                pid,
                binary_version,
                started_at: Utc::now(),
                state: ProcessState::Starting,
            },
            allocated_hardware,
            state_handle,
        }
    }

    /// Check if this process is still running
    pub fn is_running(&self) -> bool {
        matches!(self.process.state, ProcessState::Running)
    }

    /// Check if this process can be safely terminated
    pub fn can_terminate(&self) -> bool {
        matches!(
            self.process.state,
            ProcessState::Completed | ProcessState::Failed
        )
    }

    /// Get the binary version
    pub fn version(&self) -> Version {
        self.process.binary_version
    }
}

/// Process manager for all pass subprocesses
pub struct ProcessManager {
    processes: HashMap<PassId, PassProcess>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Add a process
    pub fn add_process(&mut self, process: PassProcess) {
        self.processes.insert(process.pass_id.clone(), process);
    }

    /// Get a process
    pub fn get_process(&self, pass_id: &PassId) -> Option<&PassProcess> {
        self.processes.get(pass_id)
    }

    /// Remove a process
    pub fn remove_process(&mut self, pass_id: &PassId) -> Option<PassProcess> {
        self.processes.remove(pass_id)
    }

    /// Get all processes
    pub fn all_processes(&self) -> Vec<&PassProcess> {
        self.processes.values().collect()
    }

    /// Get processes running on a specific binary version
    pub fn processes_on_version(&self, version: Version) -> Vec<&PassProcess> {
        self.processes
            .values()
            .filter(|p| p.process.binary_version == version)
            .collect()
    }

    /// Get processes that can be terminated
    pub fn terminatable_processes(&self) -> Vec<&PassProcess> {
        self.processes
            .values()
            .filter(|p| p.can_terminate())
            .collect()
    }

    /// Get process statistics
    pub fn stats(&self) -> ProcessStats {
        let total = self.processes.len();
        let running = self.processes.values().filter(|p| p.is_running()).count();
        let completed = self
            .processes
            .values()
            .filter(|p| matches!(p.process.state, ProcessState::Completed))
            .count();
        let failed = self
            .processes
            .values()
            .filter(|p| matches!(p.process.state, ProcessState::Failed))
            .count();

        // Count by version
        let mut by_version: HashMap<String, usize> = HashMap::new();
        for process in self.processes.values() {
            let version = process.process.binary_version.as_string();
            *by_version.entry(version).or_insert(0) += 1;
        }

        ProcessStats {
            total,
            running,
            completed,
            failed,
            by_version,
        }
    }
}

/// Process statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub total: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub by_version: HashMap<String, usize>,
}

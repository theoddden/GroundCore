//! Supervisor for pass subprocesses

use crate::handoff::{HandoffManager, HandoffResult};
use crate::subprocess::{PassProcess, ProcessManager};
use crate::version::{Version, VersionedBinary};
use chrono::{DateTime, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Supervisor configuration
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Path to the current binary
    pub binary_path: PathBuf,
    /// Current version
    pub current_version: Version,
    /// Schema version for shared memory
    pub schema_version: String,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            binary_path: PathBuf::from("./ground-core"),
            current_version: Version::new(0, 1, 0),
            schema_version: "1.0".to_string(),
        }
    }
}

/// Supervisor for pass subprocesses
pub struct Supervisor {
    /// Supervisor configuration
    config: SupervisorConfig,
    /// Process manager
    process_manager: ProcessManager,
    /// Handoff manager
    handoff_manager: HandoffManager,
    /// Scheduled passes
    scheduled_passes: Vec<ScheduledPass>,
    /// Supervisor start time
    started_at: DateTime<Utc>,
}

/// Scheduled pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledPass {
    pub pass_id: PassId,
    pub scheduled_time: DateTime<Utc>,
    pub satellite_id: String,
    pub duration_sec: u64,
}

impl Supervisor {
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            process_manager: ProcessManager::new(),
            handoff_manager: HandoffManager::new(),
            scheduled_passes: Vec::new(),
            started_at: Utc::now(),
        }
    }

    /// Get current version
    pub fn current_version(&self) -> Version {
        self.config.current_version
    }

    /// Schedule a pass
    pub fn schedule_pass(&mut self, pass: ScheduledPass) {
        self.scheduled_passes.push(pass);
    }

    /// Start a pass subprocess by spawning the configured binary.
    ///
    /// Arguments passed to the child:
    /// - `--pass-id <uuid>`
    /// - `--shard-region shard-<uuid>` (shared-memory region name)
    /// - `--hardware <dev1>,<dev2>,...`
    pub fn start_pass(&mut self, pass_id: PassId, hardware: Vec<String>) -> Result<PassProcess> {
        let region_id = format!("shard-{}", pass_id);
        let hardware_str = hardware.join(",");

        let child = Command::new(&self.config.binary_path)
            .arg("--pass-id")
            .arg(&pass_id)
            .arg("--shard-region")
            .arg(&region_id)
            .arg("--hardware")
            .arg(&hardware_str)
            .spawn()
            .map_err(|e| {
                ground_core::GroundStationError::Hardware(format!(
                    "Failed to spawn pass process {}: {}",
                    pass_id, e
                ))
            })?;

        let pid = child.id();
        // Leak the Child handle into a raw pid; in production you would
        // store the Child in a field and call .wait() on completion.
        std::mem::forget(child);

        let process = PassProcess::new(
            pass_id.clone(),
            pid,
            self.config.current_version,
            hardware,
            crate::subprocess::SharedMemoryHandle {
                region_id,
                size: 1024 * 1024, // 1 MB shared shard
                schema_version: self.config.schema_version.clone(),
            },
        );

        self.process_manager.add_process(process.clone());
        tracing::info!("Spawned pass {} as PID {}", pass_id, pid);
        Ok(process)
    }

    /// Signal a pass subprocess to complete gracefully.
    ///
    /// Sends SIGTERM on Unix; on Windows a TerminateProcess equivalent is used
    /// via `taskkill /PID <pid>` (best-effort).
    pub fn complete_pass(&mut self, pass_id: &PassId) -> Result<()> {
        let process = self.process_manager.get_process(pass_id).ok_or_else(|| {
            ground_core::GroundStationError::Hardware(format!("Pass {} not found", pass_id))
        })?;

        let pid = process.process.pid;
        tracing::info!(
            "Sending completion signal to pass {} (PID {})",
            pass_id,
            pid
        );

        #[cfg(unix)]
        {
            let status = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(_) => {
                    tracing::warn!(
                        "kill -TERM {} returned non-zero (process may have already exited)",
                        pid
                    );
                }
                Err(e) => {
                    return Err(ground_core::GroundStationError::Hardware(format!(
                        "Failed to send SIGTERM to PID {}: {}",
                        pid, e
                    )));
                }
            }
        }

        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();
        }

        Ok(())
    }

    /// Get process manager
    pub fn process_manager(&self) -> &ProcessManager {
        &self.process_manager
    }

    /// Get handoff manager
    pub fn handoff_manager(&mut self) -> &mut HandoffManager {
        &mut self.handoff_manager
    }

    /// Deploy a new version (zero-downtime)
    pub fn deploy(&mut self, new_binary: VersionedBinary) -> Result<HandoffResult> {
        tracing::info!(
            "Deploying version {} (current: {})",
            new_binary.version,
            self.config.current_version
        );

        // Verify binary
        if !new_binary.verify() {
            return Err(ground_core::GroundStationError::Hardware(
                "Binary verification failed".to_string(),
            ));
        }

        // Check schema compatibility
        if !new_binary.schema_compatible(&VersionedBinary {
            version: self.config.current_version,
            path: self.config.binary_path.clone(),
            checksum: String::new(),
            built_at: Utc::now(),
            schema_version: self.config.schema_version.clone(),
        }) {
            return Err(ground_core::GroundStationError::Hardware(
                "Schema version incompatible - requires migration".to_string(),
            ));
        }

        // Perform handoff
        let result = self.handoff_manager.perform_handoff(
            &self.process_manager,
            &self.config,
            &new_binary,
        )?;

        // Update configuration
        self.config.binary_path = new_binary.path;
        self.config.current_version = new_binary.version;
        self.config.schema_version = new_binary.schema_version;

        Ok(result)
    }

    /// Get supervisor statistics
    pub fn stats(&self) -> SupervisorStats {
        let process_stats = self.process_manager.stats();

        SupervisorStats {
            version: self.config.current_version,
            uptime_seconds: (Utc::now() - self.started_at).num_seconds(),
            active_passes: process_stats.running,
            completed_passes: process_stats.completed,
            failed_passes: process_stats.failed,
            scheduled_passes: self.scheduled_passes.len(),
        }
    }
}

/// Supervisor statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorStats {
    pub version: Version,
    pub uptime_seconds: i64,
    pub active_passes: usize,
    pub completed_passes: usize,
    pub failed_passes: usize,
    pub scheduled_passes: usize,
}

//! Observation tools for the embedded agent
//!
//! These provide structured read-only access to system state.
//! The agent can observe but cannot mutate without explicit approval.

use chrono::{DateTime, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// System state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    /// When this snapshot was taken
    pub captured_at: DateTime<Utc>,
    /// Active passes
    pub active_passes: Vec<PassState>,
    /// Hardware state
    pub hardware_state: HardwareState,
    /// Telemetry data
    pub telemetry: TelemetrySnapshot,
    /// Schedule state
    pub schedule_state: ScheduleState,
    /// Recent log entries
    pub recent_logs: Vec<LogEntry>,
}

/// Pass state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassState {
    pub pass_id: PassId,
    pub satellite_id: String,
    pub status: PassStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub samples_received: u64,
    pub bytes_decoded: u64,
    pub hardware_allocated: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PassStatus {
    Scheduled,
    Preparing,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Hardware state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareState {
    pub sdr_devices: Vec<DeviceState>,
    pub antennas: Vec<DeviceState>,
    pub rotators: Vec<DeviceState>,
    pub emergency_shards_available: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub device_id: String,
    pub healthy: bool,
    pub temperature: Option<f32>,
    pub utilization: f32,
    pub current_allocation: Option<PassId>,
}

/// Telemetry snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub signal_metrics: SignalMetrics,
    pub system_metrics: SystemMetrics,
    pub network_metrics: NetworkMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMetrics {
    pub average_snr: f32,
    pub average_rssi: f32,
    pub demodulator_lock_rate: f32,
    pub doppler_correction_accuracy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub disk_usage: f32,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub connections_active: u32,
}

/// Schedule state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleState {
    pub pending_passes: u64,
    pub committed_passes: u64,
    pub next_pass_time: Option<DateTime<Utc>>,
    pub scheduler_running: bool,
}

/// Log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub component: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Observation tool - provides read-only access to system state
pub trait ObservationTool: Send + Sync {
    /// Get current system state
    fn get_system_state(&self) -> Result<SystemState>;
    
    /// Get state for a specific pass
    fn get_pass_state(&self, pass_id: &PassId) -> Result<PassState>;
    
    /// Get hardware state
    fn get_hardware_state(&self) -> Result<HardwareState>;
    
    /// Get telemetry
    fn get_telemetry(&self) -> Result<TelemetrySnapshot>;
    
    /// Query historical data
    fn query_historical(&self, query: HistoricalQuery) -> Result<Vec<SystemState>>;
}

/// Historical query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalQuery {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub components: Vec<String>,
}

/// Default observation tool implementation
pub struct DefaultObservationTool {
    // In a real implementation, this would connect to the actual system
    // For now, it's a placeholder that returns mock data
}

impl DefaultObservationTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl ObservationTool for DefaultObservationTool {
    fn get_system_state(&self) -> Result<SystemState> {
        Ok(SystemState {
            captured_at: Utc::now(),
            active_passes: Vec::new(),
            hardware_state: HardwareState {
                sdr_devices: Vec::new(),
                antennas: Vec::new(),
                rotators: Vec::new(),
                emergency_shards_available: 3,
            },
            telemetry: TelemetrySnapshot {
                signal_metrics: SignalMetrics {
                    average_snr: 20.0,
                    average_rssi: -60.0,
                    demodulator_lock_rate: 0.98,
                    doppler_correction_accuracy: 0.99,
                },
                system_metrics: SystemMetrics {
                    cpu_usage: 0.45,
                    memory_usage: 0.62,
                    disk_usage: 0.34,
                    uptime_seconds: 86400,
                },
                network_metrics: NetworkMetrics {
                    bytes_received: 1_000_000_000,
                    bytes_sent: 500_000_000,
                    connections_active: 42,
                },
            },
            schedule_state: ScheduleState {
                pending_passes: 15,
                committed_passes: 8,
                next_pass_time: Some(Utc::now() + chrono::Duration::minutes(30)),
                scheduler_running: true,
            },
            recent_logs: Vec::new(),
        })
    }
    
    fn get_pass_state(&self, _pass_id: &PassId) -> Result<PassState> {
        // Placeholder implementation
        Ok(PassState {
            pass_id: _pass_id.clone(),
            satellite_id: "SAT-001".to_string(),
            status: PassStatus::InProgress,
            started_at: Some(Utc::now() - chrono::Duration::minutes(5)),
            samples_received: 10_000_000,
            bytes_decoded: 5_000_000,
            hardware_allocated: vec!["sdr-0".to_string()],
        })
    }
    
    fn get_hardware_state(&self) -> Result<HardwareState> {
        Ok(HardwareState {
            sdr_devices: vec![],
            antennas: vec![],
            rotators: vec![],
            emergency_shards_available: 3,
        })
    }
    
    fn get_telemetry(&self) -> Result<TelemetrySnapshot> {
        Ok(TelemetrySnapshot {
            signal_metrics: SignalMetrics {
                average_snr: 20.0,
                average_rssi: -60.0,
                demodulator_lock_rate: 0.98,
                doppler_correction_accuracy: 0.99,
            },
            system_metrics: SystemMetrics {
                cpu_usage: 0.45,
                memory_usage: 0.62,
                disk_usage: 0.34,
                uptime_seconds: 86400,
            },
            network_metrics: NetworkMetrics {
                bytes_received: 1_000_000_000,
                bytes_sent: 500_000_000,
                connections_active: 42,
            },
        })
    }
    
    fn query_historical(&self, _query: HistoricalQuery) -> Result<Vec<SystemState>> {
        // Placeholder implementation
        Ok(Vec::new())
    }
}

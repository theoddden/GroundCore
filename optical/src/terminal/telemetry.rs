// Telemetry streaming

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;

/// Telemetry frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    pub timestamp: DateTime<Utc>,
    pub frame_type: TelemetryType,
    pub payload: Vec<u8>,
}

/// Telemetry type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryType {
    Tracking,
    Health,
    Power,
    Thermal,
    Error,
}

/// Telemetry stream trait
pub trait TelemetryStream: Stream<Item = TelemetryFrame> + Send + Unpin {
    fn frame_type(&self) -> TelemetryType;
}

/// Batched telemetry output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBatch {
    pub batch_id: String,
    pub frames: Vec<TelemetryFrame>,
    pub created_at: DateTime<Utc>,
}

/// Telemetry batcher for efficient streaming
pub struct TelemetryBatcher {
    pending: Vec<TelemetryFrame>,
    batch_size: usize,
}

impl TelemetryBatcher {
    pub fn new(batch_size: usize) -> Self {
        Self {
            pending: Vec::new(),
            batch_size,
        }
    }

    pub fn add_frame(&mut self, frame: TelemetryFrame) {
        self.pending.push(frame);
    }

    pub fn is_ready(&self) -> bool {
        self.pending.len() >= self.batch_size
    }

    pub fn flush(&mut self) -> Option<TelemetryBatch> {
        if self.pending.is_empty() {
            return None;
        }
        let batch = TelemetryBatch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            frames: std::mem::take(&mut self.pending),
            created_at: Utc::now(),
        };
        Some(batch)
    }

    pub fn force_flush(&mut self) -> Option<TelemetryBatch> {
        self.flush()
    }
}

/// Health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub overall_health: HealthStatus,
    pub components: Vec<ComponentHealth>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub reported_at: DateTime<Utc>,
}

impl HealthReport {
    pub fn healthy() -> Self {
        Self {
            overall_health: HealthStatus::Healthy,
            components: vec![],
            warnings: vec![],
            errors: vec![],
            reported_at: Utc::now(),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.overall_health == HealthStatus::Healthy && self.errors.is_empty()
    }
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Failed,
}

/// Component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub component: String,
    pub status: HealthStatus,
    pub value: Option<f64>,
    pub unit: Option<String>,
}

impl ComponentHealth {
    pub fn new(component: String, status: HealthStatus) -> Self {
        Self {
            component,
            status,
            value: None,
            unit: None,
        }
    }

    pub fn with_value(component: String, status: HealthStatus, value: f64, unit: String) -> Self {
        Self {
            component,
            status,
            value: Some(value),
            unit: Some(unit),
        }
    }
}

/// Reset level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetLevel {
    Soft,
    Hard,
    Factory,
}

// Mission Plane - Intent-based mission planning and compilation
//
// The Mission Plane translates declarative operator intent into imperative
// satellite tasking. This is where the architectural value lives - the
// control plane that decides which terminals point where, when, and how
// to recover from failure.

pub mod compiler;
pub mod scheduler;
pub mod state_machine;
pub mod validator;

// Re-export submodule types
pub use compiler::IntentCompiler;
pub use scheduler::TaskingScheduler;
pub use state_machine::{ConstellationState, SatelliteState};
pub use validator::{PlanValidator, ValidationReport};

use crate::{
    AssetId, BiTemporal, Bytes, GeoRegion, IntentId, SatelliteId, SensorType, TaskId, TenantId,
    TimeWindow,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Mission intent - what the operator wants (declarative)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionIntent {
    pub intent_id: IntentId,
    pub objective: ObjectiveType,
    pub constraints: IntentConstraints,
    pub sla: ServiceLevelAgreement,
    pub submitted_by: TenantId,
    pub submitted_at: BiTemporal<DateTime<Utc>>,
}

/// Objective type - what the mission accomplishes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ObjectiveType {
    DataRelay {
        source: AssetId,
        destination: AssetId,
        volume: Bytes,
    },
    Observation {
        target: GeoRegion,
        sensor: SensorType,
        revisit_rate: chrono::Duration,
    },
    Custody {
        target: AssetId,
        persistence: chrono::Duration,
    },
    Downlink {
        satellite: SatelliteId,
        ground_window: TimeWindow,
    },
}

/// Constraints on the mission intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentConstraints {
    pub max_latency: Option<chrono::Duration>,
    pub min_throughput: Option<u64>, // bytes per second
    pub allowed_regions: Option<Vec<GeoRegion>>,
    pub forbidden_regions: Option<Vec<GeoRegion>>,
    pub power_budget: Option<f64>,  // watts
    pub thermal_limit: Option<f64>, // celsius
}
/// Routing decision explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub source: SatelliteId,
    pub destination: SatelliteId,
    pub selected_route: Vec<SatelliteId>,
    pub alternative_routes: Vec<Vec<SatelliteId>>,
    pub rationale: String,
}

/// Resource allocation decision explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocationDecision {
    pub satellite_id: SatelliteId,
    pub terminal_id: crate::TerminalId,
    pub task_id: TaskId,
    pub rationale: String,
}

/// Tradeoff explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeoffExplanation {
    pub tradeoff_type: TradeoffType,
    pub chosen_option: String,
    pub rejected_options: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeoffType {
    LatencyVsThroughput,
    PowerVsReliability,
    RouteComplexityVsResilience,
}

/// Compilation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompilationError {
    NoFeasibleRoute { source: String, destination: String },
    InsufficientResources { satellite_id: String },
    UnsatisfiableConstraints { reason: String },
    DeadlineMissed { required: u64, available: u64 },
    InvalidIntent(String),
    TopologyUnavailable { horizon: chrono::Duration },
    Internal(AnyhowError),
}

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFeasibleRoute {
                source,
                destination,
            } => {
                write!(
                    f,
                    "No feasible route found between {} and {}",
                    source,
                    destination
                )
            }
            Self::InsufficientResources { satellite_id } => {
                write!(f, "Insufficient resources on satellite {}", satellite_id)
            }
            Self::UnsatisfiableConstraints { reason } => {
                write!(f, "Constraints cannot be satisfied: {}", reason)
            }
            Self::DeadlineMissed {
                required,
                available,
            } => {
                write!(
                    f,
                    "Intent deadline cannot be met: required {}s, available {}s",
                    required,
                    available
                )
            }
            Self::InvalidIntent(msg) => write!(f, "Invalid intent: {}", msg),
            Self::TopologyUnavailable { horizon } => {
                write!(f, "Topology forecast unavailable for horizon {:?}", horizon)
            }
            Self::Internal(err) => write!(f, "Internal error: {}", err),
        }
    }
}

impl std::error::Error for CompilationError {}

/// Validation warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub warning_type: ValidationWarningType,
    pub message: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarningType {
    LowConfidenceRoute,
    ResourceContention,
    TimingMargin,
    RegulatoryConcern,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

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
    AssetId, BandwidthAllocation, BiTemporal, Bytes, ConfidenceScore, GeoRegion, IntentId,
    PlanId, Priority, SatelliteId, SensorType, TaskId, TenantId, TimeWindow,
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
    Internal(String),
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
                    source, destination
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
                    required, available
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

/// Service level agreement - quality of service requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLevelAgreement {
    pub priority: Priority,
    pub max_latency: Option<chrono::Duration>,
    pub min_throughput: Option<u64>,
    pub reliability_target: Option<f64>, // 0.0 to 1.0
    pub deadline: Option<chrono::DateTime<Utc>>,
}

/// Task type - what the task accomplishes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskType {
    Observation {
        target: GeoRegion,
        sensor: SensorType,
    },
    Downlink {
        ground_station: AssetId,
    },
    OpticalLinkEstablishment {
        peer_terminal: crate::TerminalId,
    },
    DataRelay {
        source: AssetId,
        destination: AssetId,
    },
    DataTransfer {
        volume: crate::Bytes,
    },
}

/// Satellite task - a single task assigned to a satellite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteTask {
    pub task_id: TaskId,
    pub satellite_id: SatelliteId,
    pub task_type: TaskType,
    pub scheduled_window: TimeWindow,
    pub dependencies: Vec<TaskId>,
    pub priority: Priority,
}

/// Link reservation - reservation of a communication link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkReservation {
    pub link_id: uuid::Uuid,
    pub source: SatelliteId,
    pub destination: SatelliteId,
    pub terminal_a: uuid::Uuid,
    pub terminal_b: uuid::Uuid,
    pub time_window: TimeWindow,
    pub bandwidth: BandwidthAllocation,
}

/// Tasking plan - the compiled plan from an intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskingPlan {
    pub plan_id: PlanId,
    pub intent_id: IntentId,
    pub satellite_tasks: Vec<SatelliteTask>,
    pub link_reservations: Vec<LinkReservation>,
    pub confidence: ConfidenceScore,
    pub compiled_at: BiTemporal<DateTime<Utc>>,
    pub valid_until: chrono::DateTime<Utc>,
}

/// Plan explanation - why the compiler made specific decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanExplanation {
    pub summary: String,
    pub routing_decisions: Vec<RoutingDecision>,
    pub resource_allocations: Vec<ResourceAllocationDecision>,
    pub tradeoffs: Vec<TradeoffExplanation>,
    pub warnings: Vec<String>,
}

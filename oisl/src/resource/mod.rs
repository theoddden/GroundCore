// Resource Plane - per-satellite as distributed system node
//
// Each satellite is a node in a distributed system with bounded resources,
// and the orchestrator schedules workloads across nodes the way Kubernetes
// schedules pods across servers. Multi-tenant isolation is first-class.
//
// Extended with Control Node Assignment Algorithm (CNAA) for satellite-side
// dynamic control node selection using TLE orbital data prediction.

pub mod manager;
pub mod allocator;
pub mod scheduler;
pub mod control_node_assignment;

pub use manager::{SatelliteNode, StorageResources, HealthMetrics, DegradationForecast};
pub use allocator::{ResourceAllocator, ResourceClaim, ResourceAllocation};
pub use scheduler::{SatelliteScheduler, AllocationId, PreemptionReason, RebalanceReport};
pub use control_node_assignment::{
    ControlNodeAssignmentAlgorithm, AssignmentPrediction, HandoffEvent,
    TleData, ControlNodeLocation, AssignmentError,
};

use crate::{
    SatelliteId, TerminalId, TaskId, TenantId, Priority, TimeWindow,
    DataRate, Bytes,
};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

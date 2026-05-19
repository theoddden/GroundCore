// Resource Plane - per-satellite as distributed system node
//
// Each satellite is a node in a distributed system with bounded resources,
// and the orchestrator schedules workloads across nodes the way Kubernetes
// schedules pods across servers. Multi-tenant isolation is first-class.
//
// Extended with Control Node Assignment Algorithm (CNAA) for satellite-side
// dynamic control node selection using TLE orbital data prediction.

pub mod allocator;
pub mod control_node_assignment;
pub mod manager;
pub mod scheduler;

pub use allocator::{ResourceAllocation, ResourceAllocator, ResourceClaim};
pub use control_node_assignment::{
    AssignmentError, AssignmentPrediction, ControlNodeAssignmentAlgorithm, ControlNodeLocation,
    HandoffEvent, TleData,
};
pub use manager::{DegradationForecast, HealthMetrics, SatelliteNode, StorageResources};
pub use scheduler::{RebalanceReport, SatelliteScheduler};
pub use crate::{AllocationId, PreemptionReason};

use crate::{Bytes, DataRate, Priority, SatelliteId, TaskId, TenantId, TerminalId, TimeWindow};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

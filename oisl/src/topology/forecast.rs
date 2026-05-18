// Topology Forecast - time-indexed graph forecast
//
// The forecast is computed by propagating orbital state forward and checking
// visibility geometry at each timestep. Resolution is configurable - for
// scheduling 24 hours ahead, 60-second resolution is fine. For real-time
// PAT coordination, sub-second resolution is required.
//
// Extended with Control Node Placement Algorithm (CNPA) (KubeSpace-inspired):
// Two-stage algorithm for optimal ground station selection to minimize
// maximum satellite-to-ground latency across constellation operation.

use crate::{
    DataRate, GeometryScore, LinkId, NodeId, NodeId as CoreNodeId, TerminalId, TimeWindow,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Topology forecast - time-indexed graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyForecast {
    pub horizon_start: DateTime<Utc>,
    pub horizon_end: DateTime<Utc>,
    pub snapshots: BTreeMap<DateTime<Utc>, GraphSnapshot>,
    pub interpolation: TopologyInterpolation,
}

/// Graph snapshot at a specific timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub timestamp: DateTime<Utc>,
    pub nodes: HashMap<NodeId, NodeState>,
    pub potential_edges: Vec<PotentialEdge>,
    pub active_links: Vec<ActiveLink>,
}

/// Node state (satellite or ground station)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: NodeId,
    pub node_type: NodeType,
    pub position: Position3D,
    pub velocity: Velocity3D,
    pub optical_terminals: Vec<TerminalId>,
}

/// Node type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Satellite { satellite_id: String },
    GroundStation { station_id: String },
}

/// 3D position (ECI coordinates)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position3D {
    pub x_km: f64,
    pub y_km: f64,
    pub z_km: f64,
}

/// 3D velocity
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Velocity3D {
    pub vx_kms: f64,
    pub vy_kms: f64,
    pub vz_kms: f64,
}

/// Potential edge - geometrically feasible link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialEdge {
    pub endpoints: (NodeId, NodeId),
    pub visibility_window: TimeWindow,
    pub geometric_quality: GeometryScore,
    pub expected_capacity: DataRate,
    pub atmospheric_attenuation: Option<f64>, // For S2T edges
    pub required_pointing_accuracy_rad: f64,
}

/// Active link - currently established link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLink {
    pub link_id: LinkId,
    pub endpoints: (TerminalId, TerminalId),
    pub phase: crate::topology::link_state::LinkPhase,
    pub phase_history: Vec<crate::topology::link_state::PhaseTransition>,
    pub metrics: crate::topology::link_state::LinkMetrics,
}

/// Topology interpolation method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopologyInterpolation {
    Linear,
    Spline,
    Nearest,
}

/// Link observation for forecast refinement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkObservation {
    pub link_id: LinkId,
    pub observed_at: DateTime<Utc>,
    pub observed_state: crate::topology::link_state::LinkPhase,
    pub observed_metrics: Option<crate::topology::link_state::LinkMetrics>,
}

/// Refinement report after observation integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementReport {
    pub observations_processed: usize,
    pub snapshots_updated: usize,
    pub forecast_confidence_delta: f64,
}

/// Topology forecaster trait
pub trait TopologyForecaster: Send + Sync {
    /// Generate topology forecast
    fn forecast(&self, horizon: Duration, resolution: Duration) -> TopologyForecast;

    /// Refine forecast with new observations
    fn refine(&mut self, new_observations: Vec<LinkObservation>) -> RefinementReport;

    /// Query topology at specific timestamp
    fn query_at(&self, timestamp: DateTime<Utc>) -> GraphSnapshot;

    /// Query topology over time window
    fn query_window(&self, window: &TimeWindow) -> Vec<GraphSnapshot>;
}

impl TopologyForecast {
    pub fn new(horizon_start: DateTime<Utc>, horizon_end: DateTime<Utc>) -> Self {
        Self {
            horizon_start,
            horizon_end,
            snapshots: BTreeMap::new(),
            interpolation: TopologyInterpolation::Linear,
        }
    }

    /// Get snapshot at timestamp, interpolating if necessary
    pub fn get_at(&self, timestamp: DateTime<Utc>) -> Option<GraphSnapshot> {
        // Exact match
        if let Some(snapshot) = self.snapshots.get(&timestamp) {
            return Some(snapshot.clone());
        }

        // Interpolate
        match self.interpolation {
            TopologyInterpolation::Nearest => {
                let before = self.snapshots.range(..=timestamp).next_back();
                let after = self.snapshots.range(timestamp..).next();

                match (before, after) {
                    (Some((_, b)), Some((_, a))) => {
                        let b_delta = (timestamp - b.timestamp).abs().num_milliseconds();
                        let a_delta = (a.timestamp - timestamp).abs().num_milliseconds();
                        if b_delta < a_delta {
                            Some(b.clone())
                        } else {
                            Some(a.clone())
                        }
                    }
                    (Some((_, b)), None) => Some(b.clone()),
                    (None, Some((_, a))) => Some(a.clone()),
                    (None, None) => None,
                }
            }
            TopologyInterpolation::Linear => {
                // For linear interpolation, we'd interpolate node positions and edge states
                // For now, fall back to nearest
                let mut forecast = self.clone();
                forecast.interpolation = TopologyInterpolation::Nearest;
                forecast.get_at(timestamp)
            }
            TopologyInterpolation::Spline => {
                // Spline interpolation would be more sophisticated
                // For now, fall back to nearest
                let mut forecast = self.clone();
                forecast.interpolation = TopologyInterpolation::Nearest;
                forecast.get_at(timestamp)
            }
        }
    }

    /// Add snapshot to forecast
    pub fn add_snapshot(&mut self, snapshot: GraphSnapshot) {
        self.snapshots.insert(snapshot.timestamp, snapshot);
    }

    /// Get forecast horizon duration
    pub fn horizon_duration(&self) -> Duration {
        self.horizon_end.signed_duration_since(self.horizon_start)
    }
}

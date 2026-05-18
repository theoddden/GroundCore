// Optical topology forecast
//
// Optical topology changes over time as satellites move in and out of each
// other's fields-of-regard. The topology forecast predicts when links will be
// available.

use crate::geometry::{PointingVector, VisibilityWindow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Optical topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpticalTopology {
    pub nodes: Vec<OpticalNode>,
    pub edges: Vec<TopologyEdge>,
    pub forecast_horizon: chrono::Duration,
    pub generated_at: DateTime<Utc>,
}

impl OpticalTopology {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            forecast_horizon: chrono::Duration::hours(6),
            generated_at: Utc::now(),
        }
    }

    pub fn add_node(&mut self, node: OpticalNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: TopologyEdge) {
        self.edges.push(edge);
    }

    pub fn find_node(&self, node_id: &str) -> Option<&OpticalNode> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }

    pub fn find_edges_from(&self, node_id: &str) -> Vec<&TopologyEdge> {
        self.edges
            .iter()
            .filter(|e| e.endpoints.0 == node_id)
            .collect()
    }

    pub fn find_edges_to(&self, node_id: &str) -> Vec<&TopologyEdge> {
        self.edges
            .iter()
            .filter(|e| e.endpoints.1 == node_id)
            .collect()
    }
}

impl Default for OpticalTopology {
    fn default() -> Self {
        Self::new()
    }
}

/// Optical node (satellite with optical terminal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpticalNode {
    pub node_id: String,
    pub satellite_id: String,
    pub terminal_id: String,
    pub vendor: String,
    pub capabilities: String, // JSON-serialized capability
    pub position: Option<SatellitePosition>,
}

impl OpticalNode {
    pub fn new(node_id: String, satellite_id: String, terminal_id: String, vendor: String) -> Self {
        Self {
            node_id,
            satellite_id,
            terminal_id,
            vendor,
            capabilities: "{}".to_string(),
            position: None,
        }
    }
}

/// Satellite position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatellitePosition {
    pub x_km: f64,
    pub y_km: f64,
    pub z_km: f64,
    pub timestamp: DateTime<Utc>,
}

/// Topology edge (potential optical link)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub edge_id: String,
    pub endpoints: (String, String),
    pub visibility_windows: Vec<VisibilityWindow>,
    pub pointing_vector: Option<PointingVector>,
    pub max_data_rate: u64,
    pub reliability_score: f64,
}

impl TopologyEdge {
    pub fn new(edge_id: String, endpoints: (String, String)) -> Self {
        Self {
            edge_id,
            endpoints,
            visibility_windows: Vec::new(),
            pointing_vector: None,
            max_data_rate: 10_000_000_000,
            reliability_score: 0.95,
        }
    }

    pub fn is_visible_at(&self, timestamp: DateTime<Utc>) -> bool {
        self.visibility_windows
            .iter()
            .any(|w| w.contains(timestamp))
    }

    pub fn next_visibility(&self, after: DateTime<Utc>) -> Option<&VisibilityWindow> {
        self.visibility_windows.iter().find(|w| w.start > after)
    }
}

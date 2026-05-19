// Control Node Assignment Algorithm (CNAA) - KubeSpace-inspired
//
// Satellite-side algorithm for dynamic control node assignment.
// Uses TLE orbital data to predict satellite positions and estimate
// distances to control nodes over a future time window, triggering
// handoffs based on distance thresholds.

use crate::topology::forecast::Position3D;
use crate::{NodeId, SatelliteId};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Control Node Assignment Algorithm
pub struct ControlNodeAssignmentAlgorithm {
    prediction_window: Duration,
    prediction_interval: Duration,
    handoff_threshold_ratio: f64,
}

impl ControlNodeAssignmentAlgorithm {
    pub fn new(
        prediction_window: Duration,
        prediction_interval: Duration,
        handoff_threshold_ratio: f64,
    ) -> Self {
        Self {
            prediction_window,
            prediction_interval,
            handoff_threshold_ratio,
        }
    }

    /// Predict optimal control node assignments over time window
    pub fn predict_assignments(
        &self,
        satellite_id: SatelliteId,
        current_position: Position3D,
        control_nodes: HashMap<NodeId, ControlNodeLocation>,
        tle_data: &TleData,
    ) -> Result<AssignmentPrediction, AssignmentError> {
        let mut predictions = Vec::new();
        let mut current_time = Utc::now();
        let end_time = current_time + self.prediction_window;

        while current_time <= end_time {
            // Predict satellite position at this time
            let satellite_pos = self.predict_position_from_tle(tle_data, current_time)?;

            // Calculate distances to all control nodes
            let mut distances: Vec<(NodeId, f64)> = control_nodes
                .iter()
                .map(|(node_id, location)| {
                    let distance = self.distance_3d(satellite_pos, location.position);
                    (node_id.clone(), distance)
                })
                .collect();

            // Sort by distance
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            predictions.push(AssignmentPoint {
                timestamp: current_time,
                satellite_position: satellite_pos,
                nearest_node: distances[0].0.clone(),
                distances: distances.into_iter().collect(),
            });

            current_time += self.prediction_interval;
        }

        // Identify handoff events
        let handoff_events = self.identify_handoff_events(&predictions);

        Ok(AssignmentPrediction {
            satellite_id,
            prediction_window: self.prediction_window,
            predictions,
            handoff_events,
        })
    }

    /// Identify handoff events from predictions
    fn identify_handoff_events(&self, predictions: &[AssignmentPoint]) -> Vec<HandoffEvent> {
        let mut events = Vec::new();
        let mut current_node = None;

        for (i, point) in predictions.iter().enumerate() {
            let nearest = point.nearest_node.clone();

            if let Some(ref prev_node) = current_node {
                if prev_node != &nearest {
                    // Check if handoff threshold is met
                    let current_distance =
                        point.distances.get(prev_node).copied().unwrap_or(f64::MAX);
                    let new_distance = point.distances.get(&nearest).copied().unwrap_or(0.0);

                    if new_distance < current_distance * (1.0 - self.handoff_threshold_ratio) {
                        events.push(HandoffEvent {
                            timestamp: point.timestamp,
                            from_node: prev_node.clone(),
                            to_node: nearest.clone(),
                        });

                        current_node = Some(nearest);
                    }
                }
            } else {
                current_node = Some(nearest);
            }
        }

        events
    }

    /// Predict satellite position from TLE data at specific time using SGP4
    fn predict_position_from_tle(
        &self,
        tle: &TleData,
        time: DateTime<Utc>,
    ) -> Result<Position3D, AssignmentError> {
        // Convert TLE data to sgp4 format
        let tle_elements =
            sgp4::Elements::from_tle(None, tle.tle_line1.as_bytes(), tle.tle_line2.as_bytes())
                .map_err(|e| {
                    AssignmentError::PredictionFailed(format!("SGP4 parse error: {}", e))
                })?;

        // Create propagator
        let propagator = sgp4::Propagator::new(tle_elements).map_err(|e| {
            AssignmentError::PredictionFailed(format!("SGP4 propagator error: {}", e))
        })?;

        // Calculate time since epoch
        let time_since_epoch = time.timestamp() as f64 - tle.epoch.timestamp() as f64;

        // Propagate to target time
        let prediction = propagator
            .propagate(time_since_epoch / 60.0) // SGP4 expects minutes
            .map_err(|e| {
                AssignmentError::PredictionFailed(format!("SGP4 propagation error: {}", e))
            })?;

        // Convert to Position3D (ECI coordinates)
        Ok(Position3D {
            x_km: prediction.position[0],
            y_km: prediction.position[1],
            z_km: prediction.position[2],
        })
    }

    /// 3D distance between two positions
    fn distance_3d(&self, a: Position3D, b: Position3D) -> f64 {
        let dx = a.x_km - b.x_km;
        let dy = a.y_km - b.y_km;
        let dz = a.z_km - b.z_km;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Get current optimal control node
    pub fn get_current_optimal_node(
        &self,
        satellite_position: Position3D,
        control_nodes: &HashMap<NodeId, ControlNodeLocation>,
    ) -> Option<NodeId> {
        control_nodes
            .iter()
            .min_by(|a, b| {
                let dist_a = self.distance_3d(satellite_position, a.1.position);
                let dist_b = self.distance_3d(satellite_position, b.1.position);
                dist_a.partial_cmp(&dist_b).unwrap()
            })
            .map(|(node_id, _)| node_id.clone())
    }
}

impl Default for ControlNodeAssignmentAlgorithm {
    fn default() -> Self {
        Self {
            prediction_window: Duration::hours(12),
            prediction_interval: Duration::seconds(60),
            handoff_threshold_ratio: 0.1, // 10% distance improvement required
        }
    }
}

/// TLE (Two-Line Element) orbital data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TleData {
    pub epoch: DateTime<Utc>,
    pub tle_line1: String,
    pub tle_line2: String,
    // Legacy fields for compatibility
    pub mean_motion_rad_min: f64,
    pub eccentricity: f64,
    pub inclination_rad: f64,
    pub raan_rad: f64,
    pub argument_of_perigee_rad: f64,
    pub mean_anomaly_rad: f64,
    pub semi_major_axis_km: f64,
}

/// Control node location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlNodeLocation {
    pub node_id: NodeId,
    pub position: Position3D,
    pub capabilities: NodeCapabilities,
}

/// Node capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub supports_rf: bool,
    pub supports_optical: bool,
    pub max_satellites: usize,
}

/// Assignment prediction over time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentPrediction {
    pub satellite_id: SatelliteId,
    pub prediction_window: Duration,
    pub predictions: Vec<AssignmentPoint>,
    pub handoff_events: Vec<HandoffEvent>,
}

/// Assignment point at specific timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentPoint {
    pub timestamp: DateTime<Utc>,
    pub satellite_position: Position3D,
    pub nearest_node: NodeId,
    pub distances: HashMap<NodeId, f64>,
}

/// Predicted handoff event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEvent {
    pub handoff_time: DateTime<Utc>,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub estimated_distance_reduction: f64,
    pub prediction_index: usize,
}

/// Assignment error
#[derive(Debug, Clone, thiserror::Error)]
pub enum AssignmentError {
    #[error("Invalid TLE data: {0}")]
    InvalidTle(String),

    #[error("Position prediction failed: {0}")]
    PredictionFailed(String),

    #[error("No control nodes available")]
    NoControlNodes,
}

// Control Node Placement Algorithm (CNPA) - KubeSpace-inspired
//
// Two-stage algorithm for optimal ground station selection:
// 1. Topology clustering to select representative topologies
// 2. K-center algorithm with local search for optimal placement
//
// This minimizes maximum satellite-to-ground latency across constellation operation.

use crate::{NodeId, Position3D, NodeType};
use crate::topology::forecast::{GraphSnapshot, TopologyForecast};
use std::collections::{HashSet, HashMap};
use serde::{Deserialize, Serialize};

/// Control Node Placement Algorithm
pub struct ControlNodePlacementAlgorithm {
    num_clusters: usize,
    local_search_iterations: usize,
}

impl ControlNodePlacementAlgorithm {
    pub fn new(num_clusters: usize, local_search_iterations: usize) -> Self {
        Self {
            num_clusters,
            local_search_iterations,
        }
    }

    /// Select optimal control nodes from candidate ground stations
    pub fn select_control_nodes(
        &self,
        forecast: &TopologyForecast,
        candidate_stations: HashSet<NodeId>,
        num_control_nodes: usize,
    ) -> Result<HashSet<NodeId>, PlacementError> {
        if candidate_stations.is_empty() {
            return Err(PlacementError::NoCandidateStations);
        }

        if num_control_nodes > candidate_stations.len() {
            return Err(PlacementError::InsufficientCandidates {
                requested: num_control_nodes,
                available: candidate_stations.len(),
            });
        }

        // Stage 1: Select representative topologies via clustering
        let representative_topologies = self.select_representatives(forecast)?;

        // Stage 2: Apply k-center algorithm with local search
        let selected_nodes = self.k_center_with_local_search(
            &representative_topologies,
            &candidate_stations,
            num_control_nodes,
        );

        Ok(selected_nodes)
    }

    /// Select representative topologies via clustering
    fn select_representatives(&self, forecast: &TopologyForecast) -> Result<Vec<GraphSnapshot>, PlacementError> {
        let snapshots: Vec<_> = forecast.snapshots.values().cloned().collect();

        if snapshots.is_empty() {
            return Err(PlacementError::NoSnapshots);
        }

        if snapshots.len() <= self.num_clusters {
            return Ok(snapshots);
        }

        // Flatten each snapshot to a feature vector
        let feature_vectors: Vec<Vec<f64>> = snapshots
            .iter()
            .map(|s| self.snapshot_to_feature_vector(s))
            .collect();

        // Simple clustering: partition by time (in production, use k-means)
        let cluster_size = snapshots.len() / self.num_clusters;
        let mut representatives = Vec::new();

        for i in 0..self.num_clusters {
            let start = i * cluster_size;
            let end = if i == self.num_clusters - 1 {
                snapshots.len()
            } else {
                (i + 1) * cluster_size
            };

            // Select snapshot closest to cluster centroid
            let cluster_snapshots = &snapshots[start..end];
            let centroid = self.compute_centroid(&feature_vectors[start..end]);
            let representative = self.find_closest_to_centroid(cluster_snapshots, &feature_vectors[start..end], &centroid);
            representatives.push(representative);
        }

        Ok(representatives)
    }

    /// Convert snapshot to feature vector
    fn snapshot_to_feature_vector(&self, snapshot: &GraphSnapshot) -> Vec<f64> {
        let mut features = Vec::new();

        // Number of nodes
        features.push(snapshot.nodes.len() as f64);

        // Number of potential edges
        features.push(snapshot.potential_edges.len() as f64);

        // Average geometric quality
        if !snapshot.potential_edges.is_empty() {
            let avg_quality: f64 = snapshot.potential_edges
                .iter()
                .map(|e| e.geometric_quality.overall_quality())
                .sum::<f64>() / snapshot.potential_edges.len() as f64;
            features.push(avg_quality);
        } else {
            features.push(0.0);
        }

        // Total expected capacity
        let total_capacity: f64 = snapshot.potential_edges
            .iter()
            .map(|e| e.expected_capacity.0 as f64)
            .sum();
        features.push(total_capacity);

        features
    }

    /// Compute centroid of feature vectors
    fn compute_centroid(&self, vectors: &[Vec<f64>]) -> Vec<f64> {
        if vectors.is_empty() {
            return vec![];
        }

        let dim = vectors[0].len();
        let mut centroid = vec![0.0; dim];

        for vector in vectors {
            for (i, &val) in vector.iter().enumerate() {
                centroid[i] += val;
            }
        }

        for val in centroid.iter_mut() {
            *val /= vectors.len() as f64;
        }

        centroid
    }

    /// Find snapshot closest to centroid
    fn find_closest_to_centroid(
        &self,
        snapshots: &[GraphSnapshot],
        vectors: &[Vec<f64>],
        centroid: &[f64],
    ) -> GraphSnapshot {
        let mut min_distance = f64::MAX;
        let mut closest_index = 0;

        for (i, vector) in vectors.iter().enumerate() {
            let distance = self.euclidean_distance(vector, centroid);
            if distance < min_distance {
                min_distance = distance;
                closest_index = i;
            }
        }

        snapshots[closest_index].clone()
    }

    /// Euclidean distance between two vectors
    fn euclidean_distance(&self, a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// K-center algorithm with local search
    fn k_center_with_local_search(
        &self,
        topologies: &[GraphSnapshot],
        candidates: &HashSet<NodeId>,
        k: usize,
    ) -> HashSet<NodeId> {
        // Greedy k-center: iteratively select station that minimizes maximum distance
        let mut selected: HashSet<NodeId> = HashSet::new();
        let mut candidates_vec: Vec<_> = candidates.iter().cloned().collect();

        // Select first station randomly
        if let Some(first) = candidates_vec.pop() {
            selected.insert(first);
        }

        // Select remaining k-1 stations
        while selected.len() < k && !candidates_vec.is_empty() {
            let best_station = self.select_best_station(topologies, &selected, candidates, &candidates_vec);
            if let Some(best) = best_station {
                selected.insert(best);
                candidates_vec.retain(|s| s != &best);
            } else {
                break;
            }
        }

        // Local search refinement
        let refined = self.local_search_refinement(topologies, selected, candidates, k);

        refined
    }

    /// Select best station to add to selected set
    fn select_best_station(
        &self,
        topologies: &[GraphSnapshot],
        selected: &HashSet<NodeId>,
        candidates: &HashSet<NodeId>,
        remaining: &[NodeId],
    ) -> Option<NodeId> {
        let mut best_station = None;
        let mut best_score = f64::MAX;

        for station in remaining {
            let mut test_selected = selected.clone();
            test_selected.insert(station.clone());

            let score = self.evaluate_placement(topologies, &test_selected);

            if score < best_score {
                best_score = score;
                best_station = Some(station.clone());
            }
        }

        best_station
    }

    /// Evaluate placement: maximum satellite-to-ground distance
    fn evaluate_placement(&self, topologies: &[GraphSnapshot], selected: &HashSet<NodeId>) -> f64 {
        let mut max_distance = 0.0;

        for topology in topologies {
            for (node_id, node_state) in &topology.nodes {
                if matches!(node_state.node_type, NodeType::Satellite { .. }) {
                    let min_distance = self.min_distance_to_control_nodes(node_state.position, topology, selected);
                    max_distance = max_distance.max(min_distance);
                }
            }
        }

        max_distance
    }

    /// Minimum distance from satellite to any control node
    fn min_distance_to_control_nodes(
        &self,
        satellite_pos: Position3D,
        topology: &GraphSnapshot,
        control_nodes: &HashSet<NodeId>,
    ) -> f64 {
        control_nodes
            .iter()
            .filter_map(|node_id| topology.nodes.get(node_id))
            .map(|node_state| self.distance_3d(satellite_pos, node_state.position))
            .filter(|&d| d < f64::MAX)
            .min()
            .unwrap_or(f64::MAX)
    }

    /// 3D distance between two positions
    fn distance_3d(&self, a: Position3D, b: Position3D) -> f64 {
        let dx = a.x_km - b.x_km;
        let dy = a.y_km - b.y_km;
        let dz = a.z_km - b.z_km;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Local search refinement
    fn local_search_refinement(
        &self,
        topologies: &[GraphSnapshot],
        selected: HashSet<NodeId>,
        candidates: &HashSet<NodeId>,
        k: usize,
    ) -> HashSet<NodeId> {
        let mut current = selected;
        let mut improved = true;

        for _ in 0..self.local_search_iterations {
            if !improved {
                break;
            }

            improved = false;

            // Try replacing each selected station with an unselected one
            for selected_station in current.clone() {
                for candidate in candidates {
                    if current.contains(candidate) {
                        continue;
                    }

                    let mut test_set = current.clone();
                    test_set.remove(&selected_station);
                    test_set.insert(candidate.clone());

                    let current_score = self.evaluate_placement(topologies, &current);
                    let test_score = self.evaluate_placement(topologies, &test_set);

                    if test_score < current_score {
                        current = test_set;
                        improved = true;
                        break;
                    }
                }

                if improved {
                    break;
                }
            }
        }

        current
    }
}

impl Default for ControlNodePlacementAlgorithm {
    fn default() -> Self {
        Self {
            num_clusters: 5,
            local_search_iterations: 10,
        }
    }
}

/// Placement error
#[derive(Debug, Clone, thiserror::Error)]
pub enum PlacementError {
    #[error("No candidate ground stations provided")]
    NoCandidateStations,

    #[error("Insufficient candidates: requested {requested}, available {available}")]
    InsufficientCandidates { requested: usize, available: usize },

    #[error("No snapshots available in forecast")]
    NoSnapshots,

    #[error("Topology analysis failed: {0}")]
    AnalysisFailed(String),
}

/// Placement evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementEvaluation {
    pub selected_nodes: HashSet<NodeId>,
    pub max_latency_km: f64,
    pub avg_latency_km: f64,
    pub evaluation_time_ms: u64,
}

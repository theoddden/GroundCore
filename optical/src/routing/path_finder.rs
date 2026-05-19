// Optical-aware path finding
//
// Optical routing must account for acquisition time, field-of-regard constraints,
// and the binary nature of optical links.

use crate::routing::OpticalTopology;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

/// Path finder for optical routing
pub struct PathFinder {
    topology: OpticalTopology,
}

impl PathFinder {
    pub fn new(topology: OpticalTopology) -> Self {
        Self { topology }
    }

    /// Find a route from source to destination
    pub fn find_route(
        &self,
        source: &str,
        destination: &str,
        constraints: RouteConstraints,
        at_time: DateTime<Utc>,
    ) -> Option<OpticalRoute> {
        // Use BFS to find shortest path
        let mut visited = std::collections::HashSet::new();
        let mut queue = VecDeque::new();
        let mut parent_map: HashMap<String, (String, String)> = HashMap::new();

        queue.push_back(source.to_string());
        visited.insert(source.to_string());

        while let Some(current) = queue.pop_front() {
            if current == destination {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = destination.to_string();
                while node != source {
                    if let Some((prev_node, edge_id)) = parent_map.get(&node) {
                        path.push((prev_node.clone(), node.clone(), edge_id.clone()));
                        node = prev_node.clone();
                    } else {
                        break;
                    }
                }
                path.reverse();
                return Some(OpticalRoute::new(path, constraints));
            }

            // Explore neighbors
            for edge in self.topology.find_edges_from(&current) {
                if !edge.is_visible_at(at_time) {
                    continue;
                }

                let neighbor = edge.endpoints.1.clone();
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    parent_map.insert(neighbor.clone(), (current.clone(), edge.edge_id.clone()));
                    queue.push_back(neighbor.clone());
                }
            }
        }

        None
    }

    /// Find all routes from source to destination
    pub fn find_all_routes(
        &self,
        source: &str,
        destination: &str,
        constraints: RouteConstraints,
        at_time: DateTime<Utc>,
    ) -> Vec<OpticalRoute> {
        // Simplified: return first route found
        if let Some(route) = self.find_route(source, destination, constraints, at_time) {
            vec![route]
        } else {
            vec![]
        }
    }
}

/// Route constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConstraints {
    pub max_hops: usize,
    pub min_data_rate: u64,
    pub max_latency_ms: u64,
    pub require_redundancy: bool,
}

impl Default for RouteConstraints {
    fn default() -> Self {
        Self {
            max_hops: 5,
            min_data_rate: 1_000_000_000,
            max_latency_ms: 100,
            require_redundancy: false,
        }
    }
}

impl RouteConstraints {
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::default()
    }
}

/// Optical route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpticalRoute {
    pub route_id: Uuid,
    pub path: Vec<(String, String, String)>, // (from, to, edge_id)
    pub constraints: RouteConstraints,
    pub estimated_latency_ms: u64,
    pub reliability_score: f64,
}

impl OpticalRoute {
    pub fn new(path: Vec<(String, String, String)>, constraints: RouteConstraints) -> Self {
        let estimated_latency_ms = path.len() as u64 * 10; // 10ms per hop
        Self {
            route_id: Uuid::new_v4(),
            path,
            constraints,
            estimated_latency_ms,
            reliability_score: 0.95,
        }
    }

    pub fn hop_count(&self) -> usize {
        self.path.len()
    }

    pub fn source(&self) -> Option<&str> {
        self.path.first().map(|(s, _, _)| s.as_str())
    }

    pub fn destination(&self) -> Option<&str> {
        self.path.last().map(|(_, d, _)| d.as_str())
    }
}

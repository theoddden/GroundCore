// Spatiotemporal Router - routing over time-varying graph
//
// Shortest-path on a static graph is Dijkstra. Shortest-path on a spatiotemporal
// graph is harder - you're routing through time as well as space. A path might
// be (A → B at T=0) → (B → C at T=120s) where B and C aren't visible at T=0.

use crate::mission::{IntentConstraints, ServiceLevelAgreement};
use crate::topology::TopologyForecast;
use crate::topology::forecast::TopologyForecaster;
use crate::{
    BandwidthAllocation, ConfidenceScore, DataRate, GeometryScore, LinkId, NodeId, PotentialEdge,
    TerminalId, TimeWindow,
};
use caching::LruCache;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Cache key for spatiotemporal routes.
/// Time is bucketed into 5-minute windows so routes valid within the same
/// window are served from cache without re-running Dijkstra.
type RouteCacheKey = (NodeId, NodeId, i64);

/// 5-minute bucket size in seconds
const ROUTE_CACHE_BUCKET_SECS: i64 = 300;

/// Spatiotemporal router
pub struct SpatiotemporalRouter {
    forecast: TopologyForecast,
    cost_model: CostModel,
    /// Memoized routes keyed by (source, destination, time_bucket)
    route_cache: LruCache<RouteCacheKey, Route>,
}

impl SpatiotemporalRouter {
    pub fn new(forecast: TopologyForecast, cost_model: CostModel) -> Self {
        Self {
            forecast,
            cost_model,
            route_cache: LruCache::new(256),
        }
    }

    /// Compute optimal route through spatiotemporal graph.
    ///
    /// Results are memoized by (source, destination, 5-minute time bucket).
    /// Identical queries within the same bucket are served from cache.
    pub fn compute_route(
        &mut self,
        source: &NodeId,
        destination: &NodeId,
        topology: &TopologyForecast,
        constraints: &IntentConstraints,
        _sla: &ServiceLevelAgreement,
    ) -> Result<Route, RoutingError> {
        let time_bucket = topology.horizon_start.timestamp() / ROUTE_CACHE_BUCKET_SECS;
        let cache_key = (source.clone(), destination.clone(), time_bucket);

        if let Some(cached) = self.route_cache.get(&cache_key) {
            tracing::debug!("Route cache hit for {:?} -> {:?}", source, destination);
            return Ok(cached);
        }
        // Use modified Dijkstra for spatiotemporal graph
        let start_time = topology.horizon_start;

        let mut visited: HashSet<(NodeId, DateTime<Utc>)> = HashSet::new();
        let mut heap = BinaryHeap::new();

        // Initial state
        heap.push(RouteState {
            cost: 0.0,
            confidence: ConfidenceScore::new(1.0),
            current_node: source.clone(),
            current_time: start_time,
            hops: vec![],
            cost_model: self.cost_model.clone(),
        });

        let mut best_route: Option<Route> = None;

        while let Some(state) = heap.pop() {
            let state_key = (state.current_node.clone(), state.current_time);

            if visited.contains(&state_key) {
                continue;
            }
            visited.insert(state_key);

            // Check if we reached destination
            if state.current_node == *destination {
                let route = self.build_route(&state, source, destination)?;

                // Validate against constraints
                if self.satisfies_constraints(&route, constraints) {
                    best_route = Some(route);
                    break;
                }
            }

            // Explore neighbors in time-varying graph
            let snapshot = topology.query_at(state.current_time);

            for edge in &snapshot.potential_edges {
                if edge.endpoints.0 != state.current_node {
                    continue;
                }

                // Check if edge is usable at current time
                if !edge.visibility_window.contains(state.current_time) {
                    continue;
                }

                // Calculate PAT overhead
                let _pat_overhead =
                    self.cost_model.pat_overhead_weight * edge.required_pointing_accuracy_rad;

                // Calculate hop cost
                let hop_cost = self.calculate_hop_cost(edge, state.current_time, &state.cost_model);

                let new_cost = state.cost + hop_cost;
                let new_time = state.current_time + Duration::seconds(30); // Assume 30s per hop
                let new_confidence = ConfidenceScore::new(
                    state.confidence.0 * edge.geometric_quality.overall_quality(),
                );

                let mut new_hops = state.hops.clone();
                new_hops.push(RoutedHop {
                    edge: edge.clone(),
                    use_window: TimeWindow::new(state.current_time, new_time),
                    data_volume: DataRate(0), // Would be set by caller
                    bandwidth_reservation: BandwidthAllocation {
                        data_rate: edge.expected_capacity,
                        valid_window: TimeWindow::new(state.current_time, new_time),
                    },
                });

                heap.push(RouteState {
                    cost: new_cost,
                    confidence: new_confidence,
                    current_node: edge.endpoints.1.clone(),
                    current_time: new_time,
                    hops: new_hops,
                    cost_model: state.cost_model.clone(),
                });
            }
        }

        let route = best_route.ok_or(RoutingError::NoRouteFound)?;

        // Cache the computed route for this (source, destination, time_bucket)
        self.route_cache.put(cache_key, route.clone());
        Ok(route)
    }

    fn build_route(
        &self,
        state: &RouteState,
        _source: &NodeId,
        _destination: &NodeId,
    ) -> Result<Route, RoutingError> {
        Ok(Route {
            hops: state.hops.clone(),
            total_cost: CostScore(state.cost),
            confidence: state.confidence,
            failure_recovery_paths: vec![], // Would compute K-shortest paths
        })
    }

    fn calculate_hop_cost(
        &self,
        edge: &PotentialEdge,
        _current_time: DateTime<Utc>,
        cost_model: &CostModel,
    ) -> f64 {
        let latency_cost = cost_model.latency_weight * 0.1; // Simplified
        let capacity_cost =
            cost_model.capacity_weight * (1.0 / (edge.expected_capacity.0 as f64 + 1.0));
        let reliability_cost =
            cost_model.reliability_weight * (1.0 - edge.geometric_quality.overall_quality());
        let pat_cost = cost_model.pat_overhead_weight * edge.required_pointing_accuracy_rad;

        latency_cost + capacity_cost + reliability_cost + pat_cost
    }

    fn satisfies_constraints(&self, route: &Route, constraints: &IntentConstraints) -> bool {
        // Check latency constraint
        if let Some(max_latency) = constraints.max_latency {
            let route_latency = route
                .hops
                .iter()
                .map(|h| h.use_window.duration())
                .sum::<Duration>();

            if route_latency > max_latency {
                return false;
            }
        }

        true
    }
}

/// Route through spatiotemporal graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub hops: Vec<RoutedHop>,
    pub total_cost: CostScore,
    pub confidence: ConfidenceScore,
    pub failure_recovery_paths: Vec<Route>, // K-shortest paths for failover
}

/// Routed hop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedHop {
    pub edge: PotentialEdge,
    pub use_window: TimeWindow,
    pub data_volume: DataRate,
    pub bandwidth_reservation: BandwidthAllocation,
}

/// Cost score
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CostScore(pub f64);

/// Cost model for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    pub latency_weight: f64,
    pub capacity_weight: f64,
    pub reliability_weight: f64,
    pub power_weight: f64,
    pub pat_overhead_weight: f64, // Time to acquire each link
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            latency_weight: 0.3,
            capacity_weight: 0.2,
            reliability_weight: 0.3,
            power_weight: 0.1,
            pat_overhead_weight: 0.1,
        }
    }
}

/// Route state for Dijkstra
#[derive(Debug, Clone)]
struct RouteState {
    cost: f64,
    confidence: ConfidenceScore,
    current_node: NodeId,
    current_time: DateTime<Utc>,
    hops: Vec<RoutedHop>,
    cost_model: CostModel,
}

impl PartialEq for RouteState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for RouteState {}

impl Ord for RouteState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for RouteState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Routing error
#[derive(Debug, Clone, thiserror::Error)]
pub enum RoutingError {
    #[error("No route found from {source} to {destination}")]
    NoRouteFound {
        source: NodeId,
        destination: NodeId,
    },

    #[error("Topology forecast unavailable")]
    TopologyUnavailable,

    #[error("Route computation failed: {0}")]
    ComputationFailed(String),

    #[error("Invalid route parameters: {0}")]
    InvalidParameters(String),
}

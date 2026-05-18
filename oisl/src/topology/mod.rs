// Topology Plane - continuously-evolving spatiotemporal graph
//
// The constellation topology is not static - it's a time-indexed graph where
// edges exist conditionally based on visibility geometry, terminal capability,
// and atmospheric conditions. Routing must account for both space and time.
//
// Extended with Control Node Placement Algorithm (CNPA) for optimal
// ground station selection to minimize satellite-to-ground latency.

pub mod forecast;
pub mod routing;
pub mod link_state;
pub mod control_node_placement;

pub use forecast::{
    TopologyForecast, GraphSnapshot, PotentialEdge, TopologyForecaster,
    TopologyInterpolation, RefinementReport, LinkObservation,
};

pub use routing::{
    SpatiotemporalRouter, Route, RoutedHop, CostModel, CostScore,
};

pub use link_state::{
    LinkPhase, LinkMetrics, ActiveLink, PhaseTransition,
    DegradationReason, LossCause, TrackingQuality,
};

pub use control_node_placement::{
    ControlNodePlacementAlgorithm, PlacementEvaluation, PlacementError,
};

use crate::{NodeId, LinkId, TerminalId, DataRate, TimeWindow, GeometryScore};
use crate::mission::IntentConstraints;
use crate::mission::ServiceLevelAgreement;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

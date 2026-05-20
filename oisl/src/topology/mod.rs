// Topology Plane - continuously-evolving spatiotemporal graph
//
// The constellation topology is not static - it's a time-indexed graph where
// edges exist conditionally based on visibility geometry, terminal capability,
// and atmospheric conditions. Routing must account for both space and time.
//
// Extended with Control Node Placement Algorithm (CNPA) for optimal
// ground station selection to minimize satellite-to-ground latency.

pub mod control_node_placement;
pub mod forecast;
pub mod link_state;
pub mod routing;

pub use forecast::{
    GraphSnapshot, LinkObservation, PotentialEdge, RefinementReport, TopologyForecast,
    TopologyForecaster, TopologyInterpolation,
};

pub use routing::{CostModel, CostScore, Route, RoutedHop, SpatiotemporalRouter};

pub use crate::{DegradationReason, LossCause, TrackingQuality};
pub use link_state::{ActiveLink, LinkMetrics, LinkPhase, PhaseTransition};

pub use control_node_placement::{
    ControlNodePlacementAlgorithm, PlacementError, PlacementEvaluation,
};

use crate::mission::ServiceLevelAgreement;
use crate::TerminalId;

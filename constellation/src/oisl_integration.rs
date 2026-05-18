//! Integration with OISL (Optical Inter-Satellite Links)

#[cfg(feature = "oisl-integration")]
use crate::manager::ConstellationSnapshot;
#[cfg(feature = "oisl-integration")]
use crate::satellite::OrbitalState;
#[cfg(feature = "oisl-integration")]
use chrono::{DateTime, Utc};
#[cfg(feature = "oisl-integration")]
use ground_core::{NodeId, SatelliteId};
#[cfg(feature = "oisl-integration")]
use oisl::topology::{NodeState, NodeType, Position3D, Velocity3D, GraphSnapshot};

#[cfg(feature = "oisl-integration")]
impl From<OrbitalState> for NodeState {
    fn from(state: OrbitalState) -> Self {
        Self {
            node_id: state.to_tracking_state().to_string(),
            node_type: NodeType::Satellite { satellite_id: "".to_string() },
            position: Position3D {
                x_km: state.position[0],
                y_km: state.position[1],
                z_km: state.position[2],
            },
            velocity: Velocity3D {
                vx_kms: state.velocity[0],
                vy_kms: state.velocity[1],
                vz_kms: state.velocity[2],
            },
            optical_terminals: Vec::new(),
        }
    }
}

#[cfg(feature = "oisl-integration")]
impl ConstellationSnapshot {
    /// Convert to OISL GraphSnapshot
    pub fn to_oisl_graph(&self) -> GraphSnapshot {
        let nodes = self.states
            .iter()
            .map(|(sat_id, state)| {
                let node_state = NodeState::from(state.clone());
                (sat_id.clone(), node_state)
            })
            .collect();

        GraphSnapshot {
            timestamp: self.timestamp,
            nodes,
            potential_edges: Vec::new(),
            active_links: Vec::new(),
        }
    }
}

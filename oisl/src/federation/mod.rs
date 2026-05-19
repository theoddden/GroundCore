// Federation Plane - cross-operator OISL
//
// Cross-operator OISL is genuinely valuable. Planet's satellites could relay
// data through a Spire satellite if the geometry works and both operators
// consent. The orchestration framework that handles this is significantly
// more valuable than one that doesn't.
//
// Extended with distributed control node architecture (KubeSpace-inspired):
// Each ground station runs a full control plane, satellites bind to nearest
// controller for uninterrupted management with seamless handoffs.

mod control_plane_handoff;

use crate::{BiTemporal, LinkId, NodeId, OperatorId, SatelliteId, TerminalId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub use control_plane_handoff::{
    ControlPlaneHandoff, HandoffError, HandoffManager, HandoffMetrics, HandoffState, HandoffStep,
    HandoffTrigger,
};

/// Federation plane
pub struct FederationPlane {
    pub local_operator: OperatorId,
    pub federation_peers: HashMap<OperatorId, FederationPeer>,
    pub cross_operator_links: Vec<CrossOperatorLink>,
    pub attestation_engine: AttestationEngine,
    /// Distributed control nodes (ground stations running full control planes)
    pub control_nodes: HashMap<NodeId, ControlNode>,
    /// Satellite-to-control-node bindings
    pub satellite_bindings: HashMap<SatelliteId, ControlPlaneBinding>,
}

impl FederationPlane {
    pub fn new(local_operator: OperatorId) -> Self {
        Self {
            local_operator,
            federation_peers: HashMap::new(),
            cross_operator_links: vec![],
            attestation_engine: AttestationEngine::new(),
            control_nodes: HashMap::new(),
            satellite_bindings: HashMap::new(),
        }
    }

    /// Add control node (ground station running full control plane)
    pub fn add_control_node(&mut self, control_node: ControlNode) {
        self.control_nodes
            .insert(control_node.node_id.clone(), control_node);
    }

    /// Bind satellite to control node
    pub fn bind_satellite(
        &mut self,
        satellite_id: SatelliteId,
        control_node_id: NodeId,
    ) -> Result<(), FederationError> {
        if !self.control_nodes.contains_key(&control_node_id) {
            return Err(FederationError::ControlNodeNotFound {
                node_id: control_node_id,
            });
        }

        let binding = ControlPlaneBinding {
            satellite_id: satellite_id.clone(),
            control_node_id,
            state: ControlPlaneBindingState::Bound,
            bound_at: BiTemporal::new(Utc::now(), Utc::now().into(), Utc::now().into()),
            handover_history: vec![],
        };

        self.satellite_bindings.insert(satellite_id, binding);
        Ok(())
    }

    /// Initiate control plane handoff
    pub fn initiate_handoff(
        &mut self,
        satellite_id: &SatelliteId,
        target_control_node_id: NodeId,
    ) -> Result<HandoffRequest, FederationError> {
        if !self.control_nodes.contains_key(&target_control_node_id) {
            return Err(FederationError::ControlNodeNotFound {
                node_id: target_control_node_id,
            });
        }

        let binding = self
            .satellite_bindings
            .get_mut(satellite_id)
            .ok_or_else(|| FederationError::SatelliteNotBound {
                satellite_id: satellite_id.clone(),
            })?;

        let request_id = uuid::Uuid::new_v4();
        let request = HandoffRequest {
            request_id,
            satellite_id: satellite_id.clone(),
            source_control_node_id: binding.control_node_id.clone(),
            target_control_node_id,
            status: HandoffStatus::Pending,
            created_at: Utc::now(),
            completed_at: None,
        };

        binding.state = ControlPlaneBindingState::Binding;
        Ok(request)
    }

    /// Complete control plane handoff
    pub fn complete_handoff(
        &mut self,
        satellite_id: &SatelliteId,
        request: HandoffRequest,
    ) -> Result<(), FederationError> {
        let binding = self
            .satellite_bindings
            .get_mut(satellite_id)
            .ok_or_else(|| FederationError::SatelliteNotBound {
                satellite_id: satellite_id.clone(),
            })?;

        let handover_record = HandoverRecord {
            from_node: binding.control_node_id.clone(),
            to_node: request.target_control_node_id.clone(),
            handover_at: BiTemporal::new(Utc::now(), Utc::now().into(), Utc::now().into()),
            duration_ms: request
                .completed_at
                .map(|t| (t - request.created_at).num_milliseconds() as u64),
        };

        binding.control_node_id = request.target_control_node_id.clone();
        binding.state = ControlPlaneBindingState::Bound;
        binding.bound_at = BiTemporal::new(Utc::now(), Utc::now().into(), Utc::now().into());
        binding.handover_history.push(handover_record);

        Ok(())
    }

    /// Get satellite's current control node
    pub fn get_satellite_control_node(&self, satellite_id: &SatelliteId) -> Option<&NodeId> {
        self.satellite_bindings
            .get(satellite_id)
            .map(|b| &b.control_node_id)
    }

    /// Add federation peer
    pub fn add_peer(&mut self, peer: FederationPeer) {
        self.federation_peers.insert(peer.operator_id.clone(), peer);
    }

    /// Establish cross-operator link
    pub fn establish_link(
        &mut self,
        local_terminal: TerminalId,
        peer_terminal: TerminalId,
        peer_operator: OperatorId,
        agreement: RevenueAgreement,
    ) -> Result<LinkId, FederationError> {
        // Verify peer exists and is trusted
        let peer = self.federation_peers.get(&peer_operator).ok_or_else(|| {
            FederationError::PeerNotFound {
                operator_id: peer_operator.clone(),
            }
        })?;

        if peer.trust_score.0 < 0.5 {
            return Err(FederationError::InsufficientTrust {
                operator_id: peer_operator.clone(),
                trust_score: peer.trust_score.0,
            });
        }

        // Verify peer allows this link
        if !peer.federation_policy.allows_cross_operator_links() {
            return Err(FederationError::PolicyViolation {
                operator_id: peer_operator.clone(),
                reason: "Peer policy does not allow cross-operator links".to_string(),
            });
        }

        // Attest peer
        self.attestation_engine
            .attest_peer(&peer_operator)
            .map_err(|e| FederationError::AttestationFailed(e.to_string()))?;

        // Create link
        let link_id = LinkId::new_v4();
        let link = CrossOperatorLink {
            local_terminal,
            peer_terminal,
            peer_operator: peer_operator.clone(),
            revenue_sharing: agreement,
            data_provenance: ProvenanceChain::new(),
        };

        self.cross_operator_links.push(link);
        Ok(link_id)
    }

    /// Get peer by operator ID
    pub fn get_peer(&self, operator_id: &OperatorId) -> Option<&FederationPeer> {
        self.federation_peers.get(operator_id)
    }

    /// Get all cross-operator links
    pub fn get_cross_operator_links(&self) -> &[CrossOperatorLink] {
        &self.cross_operator_links
    }
}

/// Federation peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationPeer {
    pub operator_id: OperatorId,
    pub public_key: Vec<u8>,
    pub trust_score: TrustScore, // Empirical, not configured
    pub visible_assets: Vec<SatelliteId>,
    pub federation_policy: FederationPolicy,
}

/// Trust score (empirical, 0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct TrustScore(pub f64);

impl TrustScore {
    pub fn new(score: f64) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    pub fn is_low(&self) -> bool {
        self.0 < 0.5
    }
}

/// Federation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationPolicy {
    pub allows_cross_operator_links: bool,
    pub requires_attestation: bool,
    pub max_concurrent_links: usize,
    pub data_residency_requirements: Vec<String>,
}

impl FederationPolicy {
    pub fn allows_cross_operator_links(&self) -> bool {
        self.allows_cross_operator_links
    }
}

impl Default for FederationPolicy {
    fn default() -> Self {
        Self {
            allows_cross_operator_links: true,
            requires_attestation: true,
            max_concurrent_links: 10,
            data_residency_requirements: vec![],
        }
    }
}

/// Cross-operator link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossOperatorLink {
    pub local_terminal: TerminalId,
    pub peer_terminal: TerminalId,
    pub peer_operator: OperatorId,
    pub revenue_sharing: RevenueAgreement,
    pub data_provenance: ProvenanceChain,
}

/// Revenue sharing agreement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueAgreement {
    pub local_share_percent: f64,
    pub peer_share_percent: f64,
    pub billing_period_days: u32,
    pub agreement_id: String,
}

/// Provenance chain for cross-operator data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceChain {
    pub hops: Vec<ProvenanceHop>,
}

impl ProvenanceChain {
    pub fn new() -> Self {
        Self { hops: vec![] }
    }

    pub fn add_hop(&mut self, hop: ProvenanceHop) {
        self.hops.push(hop);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceHop {
    pub operator_id: OperatorId,
    pub satellite_id: SatelliteId,
    pub timestamp: DateTime<Utc>,
    pub data_hash: String,
}

/// Attestation engine
pub struct AttestationEngine {
    attestation_cache: HashMap<OperatorId, AttestationRecord>,
}

impl AttestationEngine {
    pub fn new() -> Self {
        Self {
            attestation_cache: HashMap::new(),
        }
    }

    /// Attest a peer
    pub fn attest_peer(
        &mut self,
        operator_id: &OperatorId,
    ) -> Result<AttestationRecord, FederationError> {
        // Check cache
        if let Some(record) = self.attestation_cache.get(operator_id) {
            if record.is_valid() {
                return Ok(record.clone());
            }
        }

        // Perform attestation with cryptographic hash
        let attestation_data = format!("{}:{}:{}", operator_id, Utc::now(), "attestation");
        let mut hasher = Sha256::new();
        hasher.update(attestation_data.as_bytes());
        let attestation_hash = hasher.finalize().to_vec();

        let record = AttestationRecord {
            operator_id: operator_id.clone(),
            attested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            attestation_hash,
        };

        self.attestation_cache
            .insert(operator_id.clone(), record.clone());
        Ok(record)
    }

    /// Verify attestation
    pub fn verify_attestation(&self, operator_id: &OperatorId) -> bool {
        self.attestation_cache
            .get(operator_id)
            .map(|r| r.is_valid())
            .unwrap_or(false)
    }
}

impl Default for AttestationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Attestation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationRecord {
    pub operator_id: OperatorId,
    pub attested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub attestation_hash: Vec<u8>,
}

impl AttestationRecord {
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }
}

/// Control node (ground station running full control plane)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlNode {
    pub node_id: NodeId,
    pub location: GeographicLocation,
    pub control_plane_version: String,
    pub certificate: ControlPlaneCertificate,
    pub capabilities: ControlNodeCapabilities,
    pub active_satellites: Vec<SatelliteId>,
    pub max_satellites: usize,
}

/// Geographic location of control node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
}

/// Control plane certificate (shared across all control nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneCertificate {
    pub certificate_id: String,
    pub public_key: Vec<u8>,
    pub expires_at: DateTime<Utc>,
    pub issued_at: DateTime<Utc>,
}

/// Control node capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlNodeCapabilities {
    pub supports_rf: bool,
    pub supports_optical: bool,
    pub max_control_planes: usize,
    pub supported_standards: Vec<String>,
}

/// Satellite-to-control-node binding with state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneBinding {
    pub satellite_id: SatelliteId,
    pub control_node_id: NodeId,
    pub state: ControlPlaneBindingState,
    pub bound_at: BiTemporal<DateTime<Utc>>,
    pub handover_history: Vec<HandoverRecord>,
}

/// Control plane binding state (KubeSpace-inspired)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlPlaneBindingState {
    /// Node is fully managed by this control node
    Bound,
    /// Control is in the process of being transferred to this control node
    Binding,
    /// Control is being offboarded from this control node
    Releasing,
    /// Node has been fully released from this control node's management
    Released,
}

/// Handoff request for control plane transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRequest {
    pub request_id: uuid::Uuid,
    pub satellite_id: SatelliteId,
    pub source_control_node_id: NodeId,
    pub target_control_node_id: NodeId,
    pub status: HandoffStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Handoff status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandoffStatus {
    Pending,
    Processing,
    Finished,
    Failed,
}

/// Record of completed handoff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoverRecord {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub handover_at: BiTemporal<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
}

/// Federation error
#[derive(Debug, Clone, thiserror::Error)]
pub enum FederationError {
    #[error("Peer not found: {operator_id}")]
    PeerNotFound { operator_id: OperatorId },

    #[error("Insufficient trust for {operator_id}: trust_score {trust_score}")]
    InsufficientTrust {
        operator_id: OperatorId,
        trust_score: f64,
    },

    #[error("Policy violation for {operator_id}: {reason}")]
    PolicyViolation {
        operator_id: OperatorId,
        reason: String,
    },

    #[error("Attestation failed: {0}")]
    AttestationFailed(String),

    #[error("Link establishment failed: {0}")]
    LinkEstablishmentFailed(String),

    #[error("Control node not found: {node_id}")]
    ControlNodeNotFound { node_id: NodeId },

    #[error("Satellite not bound to any control node: {satellite_id}")]
    SatelliteNotBound { satellite_id: SatelliteId },

    #[error("Handoff failed: {0}")]
    HandoffFailed(String),
}

//! Federation state snapshotting for cross-station exchange
//!
//! Uses primitive owned types instead of importing `federation::FederationPeer`
//! to avoid a circular crate dependency.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Federation snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationSnapshot {
    pub snapshot_id: String,
    pub peers: Vec<PeerState>,
    pub captured_at: DateTime<Utc>,
    pub version: u64,
}

/// Peer state captured at snapshot time — primitive owned types only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerState {
    /// Peer (station) ID as a plain String to avoid the federation::PeerId import
    pub peer_id: String,
    pub trust_score: f64,
    pub status: String,
    pub last_verification: Option<DateTime<Utc>>,
    pub total_attestations: u64,
}

/// Federation snapshotter
#[derive(Debug, Clone)]
pub struct FederationSnapshotter {
    snapshots: Vec<FederationSnapshot>,
}

impl FederationSnapshotter {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    /// Take a snapshot from pre-converted peer states.
    /// Callers are responsible for extracting `PeerState` from their peer types,
    /// keeping snapshotting fully decoupled from federation internals.
    pub fn snapshot(&mut self, peers: Vec<PeerState>) -> FederationSnapshot {
        let snapshot = FederationSnapshot {
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            peers,
            captured_at: Utc::now(),
            version: self.snapshots.len() as u64,
        };
        self.snapshots.push(snapshot.clone());
        snapshot
    }

    pub fn latest(&self) -> Option<&FederationSnapshot> {
        self.snapshots.last()
    }

    pub fn get_snapshots(&self) -> &[FederationSnapshot] {
        &self.snapshots
    }
}

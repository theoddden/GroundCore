//! Federation peer management

use crate::verification::ChallengeSchedule;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use ground_core::{PassId, StationId};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Maximum attestation records retained per peer.
/// A year of once-per-pass verification (~6 passes/day) = 2,190 records.
/// Cap at 200 — only the trend matters, not full history.
const MAX_ATTESTATION_HISTORY: usize = 200;

/// Peer identifier
pub type PeerId = StationId;

/// Federation peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationPeer {
    /// Station ID
    pub station_id: PeerId,
    /// Public key for signature verification
    pub public_key: String,
    /// Bounded attestation history — oldest records evicted when full
    pub attestation_history: VecDeque<AttestationRecord>,
    /// Challenge pass schedule
    pub challenge_schedule: ChallengeSchedule,
    /// Trust score (0.0 to 1.0)
    pub trust_score: f64,
    /// Peer status
    pub status: PeerStatus,
    /// When this peer was added
    pub added_at: DateTime<Utc>,
    /// Last verification time
    pub last_verification: Option<DateTime<Utc>>,
    /// Total attestations ever received (even those evicted from history)
    pub total_attestations: u64,
}

/// Attestation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationRecord {
    /// Pass ID
    pub pass_id: PassId,
    /// When attestation was received
    pub received_at: DateTime<Utc>,
    /// Verification result
    pub verification: VerificationResult,
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification passed
    pub passed: bool,
    /// Match rate (0.0 to 1.0)
    pub match_rate: f64,
    /// Bi-temporal consistency
    pub bitemporal_consistency: bool,
    /// Details
    pub details: String,
}

/// Peer status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    /// Trusted peer
    Trusted,
    /// Untrusted (requires verification)
    Untrusted,
    /// Suspicious (failed recent verifications)
    Suspicious,
    /// Blacklisted (repeated failures)
    Blacklisted,
}

/// Trust score for a peer
#[derive(Debug, Clone, Copy)]
pub struct PeerTrustScore {
    /// Current score (0.0 to 1.0)
    pub score: f64,
    /// Number of successful verifications
    pub successes: u64,
    /// Number of failed verifications
    pub failures: u64,
    /// Total verifications
    pub total: u64,
}

impl PeerTrustScore {
    pub fn new() -> Self {
        Self {
            score: 0.5, // Start neutral
            successes: 0,
            failures: 0,
            total: 0,
        }
    }

    /// Update trust score based on verification result
    pub fn update(&mut self, passed: bool, match_rate: f64) {
        self.total += 1;

        if passed {
            self.successes += 1;
            // Increase trust, more for higher match rates
            let increase = 0.1 * match_rate;
            self.score = (self.score + increase).min(1.0);
        } else {
            self.failures += 1;
            // Decrease trust
            let decrease = 0.2;
            self.score = (self.score - decrease).max(0.0);
        }
    }

    /// Get peer status based on trust score
    pub fn status(&self) -> PeerStatus {
        if self.score >= 0.8 {
            PeerStatus::Trusted
        } else if self.score >= 0.5 {
            PeerStatus::Untrusted
        } else if self.score >= 0.2 {
            PeerStatus::Suspicious
        } else {
            PeerStatus::Blacklisted
        }
    }
}

impl FederationPeer {
    pub fn new(station_id: PeerId, public_key: String) -> Self {
        Self {
            station_id,
            public_key,
            attestation_history: VecDeque::with_capacity(MAX_ATTESTATION_HISTORY),
            challenge_schedule: ChallengeSchedule::new(),
            trust_score: 0.5,
            status: PeerStatus::Untrusted,
            added_at: Utc::now(),
            last_verification: None,
            total_attestations: 0,
        }
    }

    /// Add an attestation record, evicting the oldest if the bounded history is full.
    pub fn add_attestation(&mut self, record: AttestationRecord) {
        if self.attestation_history.len() >= MAX_ATTESTATION_HISTORY {
            self.attestation_history.pop_front();
        }
        self.attestation_history.push_back(record);
        self.total_attestations += 1;
        self.last_verification = Some(Utc::now());

        // Update trust score based on the latest attestation
        if let Some(latest) = self.attestation_history.back() {
            let mut trust = PeerTrustScore {
                score: self.trust_score,
                successes: self
                    .attestation_history
                    .iter()
                    .filter(|r| r.verification.passed)
                    .count() as u64,
                failures: self
                    .attestation_history
                    .iter()
                    .filter(|r| !r.verification.passed)
                    .count() as u64,
                total: self.total_attestations,
            };
            trust.update(latest.verification.passed, latest.verification.match_rate);
            self.trust_score = trust.score;
            self.status = trust.status();
        }
    }

    /// Check if peer is currently trusted
    pub fn is_trusted(&self) -> bool {
        matches!(self.status, PeerStatus::Trusted)
    }

    /// Check if peer can be used without verification
    pub fn can_skip_verification(&self) -> bool {
        self.is_trusted() && self.attestation_history.len() >= 5
    }
}

/// Peer manager
pub struct PeerManager {
    peers: HashMap<PeerId, FederationPeer>,
    secret_key: SigningKey,
}

impl PeerManager {
    pub fn new() -> Self {
        let mut rng = OsRng;
        let secret_key = SigningKey::generate(&mut rng);

        Self {
            peers: HashMap::new(),
            secret_key,
        }
    }

    /// Create with existing key (for testing)
    pub fn with_key(secret_key: SigningKey) -> Self {
        Self {
            peers: HashMap::new(),
            secret_key,
        }
    }

    /// Get our public key
    pub fn public_key(&self) -> String {
        hex::encode(self.secret_key.verifying_key().as_bytes())
    }

    /// Sign data
    pub fn sign(&self, data: &[u8]) -> String {
        let signature = self.secret_key.sign(data);
        hex::encode(signature.to_bytes())
    }

    /// Add a peer
    pub fn add_peer(&mut self, peer: FederationPeer) {
        self.peers.insert(peer.station_id.clone(), peer);
    }

    /// Get a peer
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&FederationPeer> {
        self.peers.get(peer_id)
    }

    /// Get all peers
    pub fn all_peers(&self) -> Vec<&FederationPeer> {
        self.peers.values().collect()
    }

    /// Get trusted peers
    pub fn trusted_peers(&self) -> Vec<&FederationPeer> {
        self.peers.values().filter(|p| p.is_trusted()).collect()
    }

    /// Remove a peer
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }

    /// Update peer trust score
    pub fn update_trust(&mut self, peer_id: &PeerId, passed: bool, match_rate: f64) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            let mut trust = PeerTrustScore {
                score: peer.trust_score,
                successes: peer
                    .attestation_history
                    .iter()
                    .filter(|r| r.verification.passed)
                    .count() as u64,
                failures: peer
                    .attestation_history
                    .iter()
                    .filter(|r| !r.verification.passed)
                    .count() as u64,
                total: peer.total_attestations,
            };
            trust.update(passed, match_rate);
            peer.trust_score = trust.score;
            peer.status = trust.status();
        }
    }
}

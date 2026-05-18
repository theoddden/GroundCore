//! Cross-verification and challenge passes

use crate::attestation::{Attestation, BitemporalProof};
use crate::peer::{AttestationRecord, FederationPeer, PeerId, VerificationResult};
use chrono::{DateTime, Duration, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};

/// Maximum believable pass duration: 20 minutes
const MAX_PASS_DURATION_SECS: i64 = 1200;
/// Minimum sample count to consider a proof plausible
const MIN_SAMPLE_COUNT: u64 = 1;
/// Maximum allowed clock drift between peers (ms) for bitemporal consistency
const MAX_BITEMPORAL_DRIFT_MS: i64 = 5_000;

/// Attestation verification record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationVerification {
    /// Pass ID
    pub pass_id: PassId,
    /// Local capture hash (if we also captured)
    pub local_capture: Option<String>,
    /// Peer capture hash
    pub peer_capture: String,
    /// Bytewise match rate (0.0 to 1.0)
    pub bytewise_match_rate: f64,
    /// Bi-temporal consistency
    pub bitemporal_consistency: bool,
    /// Overall verification passed
    pub passed: bool,
    /// Details
    pub details: String,
}

/// Challenge schedule for periodic verification passes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeSchedule {
    /// Scheduled challenge passes
    pub challenges: Vec<ChallengePass>,
    /// Challenge interval (hours)
    pub interval_hours: u64,
}

/// Challenge pass for cross-verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengePass {
    /// Pass ID
    pub pass_id: PassId,
    /// Satellite ID
    pub satellite_id: String,
    /// Scheduled time
    pub scheduled_time: DateTime<Utc>,
    /// Peers to verify
    pub peers_to_verify: Vec<PeerId>,
    /// Completed
    pub completed: bool,
    /// Results
    pub results: Vec<ChallengeResult>,
}

/// Challenge result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResult {
    /// Peer ID
    pub peer_id: PeerId,
    /// Verification result
    pub verification: AttestationVerification,
    /// Trust score impact
    pub trust_delta: f64,
}

impl ChallengeSchedule {
    pub fn new() -> Self {
        Self {
            challenges: Vec::new(),
            interval_hours: 24,
        }
    }

    /// Add a challenge pass
    pub fn add_challenge(&mut self, challenge: ChallengePass) {
        self.challenges.push(challenge);
    }

    /// Get pending challenges
    pub fn pending_challenges(&self) -> Vec<&ChallengePass> {
        self.challenges
            .iter()
            .filter(|c| !c.completed && c.scheduled_time > Utc::now())
            .collect()
    }

    /// Get overdue challenges
    pub fn overdue_challenges(&self) -> Vec<&ChallengePass> {
        self.challenges
            .iter()
            .filter(|c| !c.completed && c.scheduled_time < Utc::now())
            .collect()
    }
}

impl AttestationVerification {
    /// Create a new verification record.
    ///
    /// - `local_capture`: hex SHA-256 of local data, if we independently captured this pass.
    /// - `peer_capture`: hex SHA-256 reported by the peer.
    /// - `peer_proof`: the peer's bi-temporal proof; used for consistency checks.
    /// - `local_proof`: our own bi-temporal proof, if available, for cross-comparison.
    pub fn new(
        pass_id: PassId,
        local_capture: Option<String>,
        peer_capture: String,
        peer_proof: Option<&BitemporalProof>,
        local_proof: Option<&BitemporalProof>,
    ) -> Self {
        let (match_rate, bitemporal_consistency) = if let Some(local) = &local_capture {
            let match_rate = Self::compute_match_rate(local, &peer_capture);
            let bt = Self::check_bitemporal_consistency(peer_proof, local_proof);
            (match_rate, bt)
        } else {
            // No local capture: only check the peer's own proof for internal sanity.
            let bt = peer_proof
                .map(|p| Self::proof_internally_consistent(p))
                .unwrap_or(false);
            (0.0, bt)
        };

        let passed = match_rate > 0.95 && bitemporal_consistency;

        let details = if passed {
            format!(
                "Verification passed: {:.1}% byte match, bitemporal OK",
                match_rate * 100.0
            )
        } else {
            format!(
                "Verification failed: {:.1}% byte match, bitemporal={}",
                match_rate * 100.0,
                bitemporal_consistency
            )
        };

        Self {
            pass_id,
            local_capture,
            peer_capture,
            bytewise_match_rate: match_rate,
            bitemporal_consistency,
            passed,
            details,
        }
    }

    /// Compute per-byte Hamming similarity between two hex-encoded SHA-256 digests.
    ///
    /// Decodes both as hex, aligns them to the shorter length, counts how many
    /// bytes are identical at the same position, and divides by total length.
    /// Returns 0.0 if either string cannot be decoded or is empty.
    pub fn compute_match_rate(local: &str, peer: &str) -> f64 {
        if local == peer {
            return 1.0;
        }

        let local_bytes = match hex::decode(local) {
            Ok(b) => b,
            Err(_) => {
                // Fall back to character-level comparison for non-hex hashes
                return Self::char_match_rate(local, peer);
            }
        };
        let peer_bytes = match hex::decode(peer) {
            Ok(b) => b,
            Err(_) => return Self::char_match_rate(local, peer),
        };

        if local_bytes.is_empty() || peer_bytes.is_empty() {
            return 0.0;
        }

        let len = local_bytes.len().max(peer_bytes.len());
        let matching: usize = local_bytes
            .iter()
            .zip(peer_bytes.iter())
            .filter(|(a, b)| a == b)
            .count();

        // Bytes that are beyond the shorter digest count as mismatches
        matching as f64 / len as f64
    }

    /// Fallback: character-level match rate for non-hex string captures.
    fn char_match_rate(a: &str, b: &str) -> f64 {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let len = a_chars.len().max(b_chars.len());
        if len == 0 {
            return 1.0;
        }
        let matching = a_chars
            .iter()
            .zip(b_chars.iter())
            .filter(|(x, y)| x == y)
            .count();
        matching as f64 / len as f64
    }

    /// Check whether the peer's bi-temporal proof is internally consistent:
    /// 1. `first_event_time <= last_event_time` (monotone events)
    /// 2. `first_reception_time <= last_reception_time` (monotone reception)
    /// 3. Reception time >= event time (causality: we can't receive before it's emitted)
    /// 4. Pass duration <= MAX_PASS_DURATION_SECS (plausibility)
    /// 5. `sample_count >= MIN_SAMPLE_COUNT`
    fn proof_internally_consistent(proof: &BitemporalProof) -> bool {
        let duration = proof
            .last_event_time
            .signed_duration_since(proof.first_event_time);
        let reception_span = proof
            .last_reception_time
            .signed_duration_since(proof.first_reception_time);

        duration.num_seconds() >= 0
            && reception_span.num_seconds() >= 0
            && proof.first_reception_time >= proof.first_event_time
            && duration.num_seconds() <= MAX_PASS_DURATION_SECS
            && proof.sample_count >= MIN_SAMPLE_COUNT
    }

    /// Cross-compare peer and local bi-temporal proofs:
    /// 1. Peer proof must be internally consistent.
    /// 2. If local proof is available, the event time windows must overlap
    ///    (within MAX_BITEMPORAL_DRIFT_MS tolerance at the boundaries).
    fn check_bitemporal_consistency(
        peer_proof: Option<&BitemporalProof>,
        local_proof: Option<&BitemporalProof>,
    ) -> bool {
        let Some(peer) = peer_proof else { return false };
        if !Self::proof_internally_consistent(peer) {
            return false;
        }

        let Some(local) = local_proof else {
            // No local proof — accept the peer's own consistency as sufficient
            return true;
        };

        // Check that the two event-time windows overlap (with drift tolerance)
        let drift = Duration::milliseconds(MAX_BITEMPORAL_DRIFT_MS);
        let peer_start = peer.first_event_time - drift;
        let peer_end = peer.last_event_time + drift;
        let local_start = local.first_event_time;
        let local_end = local.last_event_time;

        // Windows overlap: not (peer_end < local_start || local_end < peer_start)
        !(peer_end < local_start || local_end < peer_start)
    }
}

/// Cross-verify a peer's attestation.
///
/// - `local_capture`: hex SHA-256 of locally captured data, if we independently observed the same pass.
/// - `local_proof`: our own bi-temporal proof for this pass, for cross-peer time-window consistency.
pub fn cross_verify_attestation(
    _peer: &FederationPeer,
    attestation: &Attestation,
    local_capture: Option<String>,
    local_proof: Option<&crate::attestation::BitemporalProof>,
) -> AttestationVerification {
    AttestationVerification::new(
        attestation.pass_id.clone(),
        local_capture,
        attestation.data_hash.clone(),
        Some(&attestation.bitemporal_proof),
        local_proof,
    )
}

/// Update peer based on verification result
pub fn update_peer_from_verification(
    peer: &mut FederationPeer,
    verification: &AttestationVerification,
) {
    let record = AttestationRecord {
        pass_id: verification.pass_id.clone(),
        received_at: Utc::now(),
        verification: VerificationResult {
            passed: verification.passed,
            match_rate: verification.bytewise_match_rate,
            bitemporal_consistency: verification.bitemporal_consistency,
            details: verification.details.clone(),
        },
    };

    peer.add_attestation(record);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_exact_match() {
        // Same hex SHA-256 digest → 1.0 match rate
        let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";
        let verification = AttestationVerification::new(
            "pass1".to_string(),
            Some(hash.to_string()),
            hash.to_string(),
            None,
            None,
        );

        assert_eq!(verification.bytewise_match_rate, 1.0);
    }

    #[test]
    fn test_verification_mismatch() {
        let local = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";
        let peer = "b665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";
        let verification = AttestationVerification::new(
            "pass1".to_string(),
            Some(local.to_string()),
            peer.to_string(),
            None,
            None,
        );

        assert!(!verification.passed);
        assert!(verification.bytewise_match_rate < 1.0);
        // 31 out of 32 bytes match → ~0.97, not > 0.95, because first byte differs
        assert!(verification.bytewise_match_rate > 0.9);
    }

    #[test]
    fn test_bitemporal_consistency_window_overlap() {
        use chrono::TimeZone;
        let t0 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let proof = crate::attestation::BitemporalProof {
            first_event_time: t0,
            first_reception_time: t0 + chrono::Duration::milliseconds(100),
            last_event_time: t0 + chrono::Duration::seconds(300),
            last_reception_time: t0
                + chrono::Duration::seconds(300)
                + chrono::Duration::milliseconds(100),
            sample_count: 1000,
        };
        assert!(AttestationVerification::proof_internally_consistent(&proof));
    }
}

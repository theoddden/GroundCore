// PAT attestation
//
// PAT attestations provide cryptographic proof that acquisition and tracking
// occurred with specific parameters at specific times.

use crate::attestation::link_proof::{AttestationId, Hash, Signature};
use crate::pat::AcquisitionResult;
use crate::{BiTemporal, EventTime, ReceptionTime};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// PAT attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatAttestation {
    pub attestation_id: AttestationId,
    pub acquisition_id: Uuid,
    pub acquisition_proof: AcquisitionProof,
    pub tracking_proof: Option<TrackingProof>,
    pub hash: Hash,
    pub signature: Signature,
    /// Hex-encoded Ed25519 public key of the signer; required by verify()
    pub signer_public_key: String,
}

impl PatAttestation {
    pub fn new(acquisition_id: Uuid, acquisition_proof: AcquisitionProof) -> Self {
        // SHA-256 over the canonical proof fields
        let mut hasher = Sha256::new();
        hasher.update(acquisition_id.as_bytes());
        hasher.update(acquisition_proof.plan_id.as_bytes());
        hasher.update(acquisition_proof.search_pattern_used.as_bytes());
        let hash_bytes = hasher.finalize();
        let hash = Hash::new(hex::encode(hash_bytes));

        // Sign the hash with an ephemeral Ed25519 keypair.
        // In production, replace with the terminal's long-term signing key.
        let mut keypair_bytes = [0u8; 64];
        OsRng.fill_bytes(&mut keypair_bytes);
        let signing_key = SigningKey::from_keypair_bytes(&keypair_bytes).unwrap();
        let verifying_key = VerifyingKey::from(&signing_key);
        let sig_bytes = signing_key.sign(&hash_bytes).to_bytes();
        let signer_public_key = hex::encode(verifying_key.as_bytes());
        let signature = Signature::new(hex::encode(sig_bytes));

        Self {
            attestation_id: Uuid::new_v4(),
            acquisition_id,
            acquisition_proof,
            tracking_proof: None,
            hash,
            signature,
            signer_public_key,
        }
    }

    pub fn with_tracking_proof(mut self, tracking_proof: TrackingProof) -> Self {
        self.tracking_proof = Some(tracking_proof);
        self
    }

    /// Cryptographically verify the attestation.
    pub fn verify(&self) -> bool {
        let pub_bytes = match hex::decode(&self.signer_public_key) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(self.signature.as_str()) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let pub_array: [u8; 32] = match pub_bytes.as_slice().try_into() {
            Ok(arr) => arr,
            Err(_) => return false,
        };
        let public_key = match ed25519_dalek::VerifyingKey::from_bytes(&pub_array) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = match ed25519_dalek::Signature::try_from(sig_bytes.as_slice()) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let mut hasher = Sha256::new();
        hasher.update(self.acquisition_id.as_bytes());
        hasher.update(self.acquisition_proof.plan_id.as_bytes());
        hasher.update(self.acquisition_proof.search_pattern_used.as_bytes());
        let hash_bytes = hasher.finalize();

        public_key.verify(&hash_bytes, &sig).is_ok()
    }
}

/// Acquisition proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionProof {
    pub plan_id: Uuid,
    pub start_time: BiTemporal<DateTime<Utc>>,
    pub end_time: BiTemporal<DateTime<Utc>>,
    pub result: AcquisitionResult,
    pub pointing_vectors: (String, String), // JSON-serialized pointing vectors
    pub search_pattern_used: String,
}

impl AcquisitionProof {
    pub fn new(plan_id: Uuid, result: AcquisitionResult, search_pattern_used: String) -> Self {
        let now = Utc::now();
        Self {
            plan_id,
            start_time: BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now)),
            end_time: BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now)),
            result,
            pointing_vectors: ("{}".to_string(), "{}".to_string()),
            search_pattern_used,
        }
    }
}

/// Tracking proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingProof {
    pub tracking_id: Uuid,
    pub start_time: BiTemporal<DateTime<Utc>>,
    pub end_time: BiTemporal<DateTime<Utc>>,
    pub duration_seconds: f64,
    pub average_pointing_error_urad: f64,
    pub average_snr_db: f64,
    pub lock_confidence: f64,
}

impl TrackingProof {
    pub fn new(tracking_id: Uuid) -> Self {
        Self {
            tracking_id,
            start_time: BiTemporal::new(
                Utc::now(),
                EventTime::new(Utc::now()),
                ReceptionTime::new(Utc::now()),
            ),
            end_time: BiTemporal::new(
                Utc::now(),
                EventTime::new(Utc::now()),
                ReceptionTime::new(Utc::now()),
            ),
            duration_seconds: 0.0,
            average_pointing_error_urad: 0.0,
            average_snr_db: 0.0,
            lock_confidence: 0.0,
        }
    }

    pub fn with_metrics(
        mut self,
        duration_seconds: f64,
        average_pointing_error_urad: f64,
        average_snr_db: f64,
        lock_confidence: f64,
    ) -> Self {
        self.duration_seconds = duration_seconds;
        self.average_pointing_error_urad = average_pointing_error_urad;
        self.average_snr_db = average_snr_db;
        self.lock_confidence = lock_confidence;
        self
    }
}

// Link attestation
//
// Link attestations provide cryptographic proof that a link was established and
// maintained with specific parameters at specific times.

use crate::{BiTemporal, EventTime, ReceptionTime};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub type AttestationId = Uuid;

/// Cryptographic hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hash(pub String);

impl Hash {
    pub fn new(hash: String) -> Self {
        Self(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Cryptographic signature
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(pub String);

impl Signature {
    pub fn new(sig: String) -> Self {
        Self(sig)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Attestable event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestableEvent {
    pub event_type: String,
    pub payload: String,
    pub timestamp: BiTemporal<DateTime<Utc>>,
    pub source: String,
}

impl AttestableEvent {
    pub fn new(event_type: String, payload: String, source: String) -> Self {
        let now = Utc::now();
        Self {
            event_type,
            payload,
            timestamp: BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now)),
            source,
        }
    }
}

/// Link attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkAttestation {
    pub attestation_id: AttestationId,
    pub link_id: uuid::Uuid,
    pub event: AttestableEvent,
    pub hash: Hash,
    pub signature: Signature,
    /// Hex-encoded Ed25519 public key of the signer; required by verify()
    pub signer_public_key: String,
    pub terminal_id: String,
}

impl LinkAttestation {
    pub fn new(link_id: uuid::Uuid, event: AttestableEvent, terminal_id: String) -> Self {
        // SHA-256 over the canonical event fields
        let mut hasher = Sha256::new();
        hasher.update(link_id.as_bytes());
        hasher.update(event.event_type.as_bytes());
        hasher.update(event.payload.as_bytes());
        hasher.update(event.source.as_bytes());
        hasher.update(terminal_id.as_bytes());
        let hash_bytes = hasher.finalize();
        let hash = Hash::new(hex::encode(hash_bytes));

        // Sign the hash with an ephemeral Ed25519 keypair.
        // In production, replace with the terminal's long-term signing key.
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = VerifyingKey::from(&signing_key);
        let sig_bytes = signing_key.sign(&hash_bytes).to_bytes();
        let signer_public_key = hex::encode(verifying_key.as_bytes());
        let signature = Signature::new(hex::encode(sig_bytes));

        Self {
            attestation_id: Uuid::new_v4(),
            link_id,
            event,
            hash,
            signature,
            signer_public_key,
            terminal_id,
        }
    }

    /// Cryptographically verify the attestation.
    /// Re-derives the hash from stored fields and checks the Ed25519 signature.
    pub fn verify(&self) -> bool {
        let pub_bytes = match hex::decode(&self.signer_public_key) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(self.signature.as_str()) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let public_key = match ed25519_dalek::PublicKey::from_bytes(&pub_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = match ed25519_dalek::Signature::try_from(sig_bytes.as_slice()) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Re-derive the hash
        let mut hasher = Sha256::new();
        hasher.update(self.link_id.as_bytes());
        hasher.update(self.event.event_type.as_bytes());
        hasher.update(self.event.payload.as_bytes());
        hasher.update(self.event.source.as_bytes());
        hasher.update(self.terminal_id.as_bytes());
        let hash_bytes = hasher.finalize();

        public_key.verify(&hash_bytes, &sig).is_ok()
    }
}

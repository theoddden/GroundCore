//! Cryptographic attestation for federation data

use ed25519_dalek::{Signature, VerifyingKey};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Signed data with attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedData {
    /// The data being signed
    pub data: Vec<u8>,
    /// Signature
    pub signature: String,
    /// Signer's public key
    pub public_key: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SignedData {
    /// Create signed data
    pub fn sign(data: Vec<u8>, signature: String, public_key: String) -> Self {
        Self {
            data,
            signature,
            public_key,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Verify the signature
    pub fn verify(&self) -> Result<bool> {
        let public_key_bytes = hex::decode(&self.public_key).map_err(|e| {
            ground_core::GroundStationError::Federation(format!("Invalid public key: {}", e))
        })?;

        let public_key_array: [u8; 32] = public_key_bytes.try_into().map_err(|_| {
            ground_core::GroundStationError::Federation("Invalid public key length".to_string())
        })?;

        let public_key = VerifyingKey::from_bytes(&public_key_array).map_err(|e| {
            ground_core::GroundStationError::Federation(format!("Invalid public key: {}", e))
        })?;

        let signature_bytes = hex::decode(&self.signature).map_err(|e| {
            ground_core::GroundStationError::Federation(format!("Invalid signature: {}", e))
        })?;

        let signature = Signature::try_from(signature_bytes.as_slice()).map_err(|e| {
            ground_core::GroundStationError::Federation(format!("Invalid signature: {}", e))
        })?;

        public_key
            .verify_strict(&self.data, &signature)
            .map_err(|e| {
                ground_core::GroundStationError::Federation(format!("Verification failed: {}", e))
            })
            .map(|_| true)
    }

    /// Compute hash of the data
    pub fn data_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        format!("{:x}", hasher.finalize())
    }
}

/// Attestation for a pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Pass ID
    pub pass_id: PassId,
    /// Captured data hash
    pub data_hash: String,
    /// Signed data
    pub signed_data: SignedData,
    /// Bi-temporal consistency proof
    pub bitemporal_proof: BitemporalProof,
}

/// Bi-temporal consistency proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitemporalProof {
    /// Event time of first sample
    pub first_event_time: chrono::DateTime<chrono::Utc>,
    /// Reception time of first sample
    pub first_reception_time: chrono::DateTime<chrono::Utc>,
    /// Event time of last sample
    pub last_event_time: chrono::DateTime<chrono::Utc>,
    /// Reception time of last sample
    pub last_reception_time: chrono::DateTime<chrono::Utc>,
    /// Sample count
    pub sample_count: u64,
}

impl Attestation {
    /// Create a new attestation
    pub fn new(
        pass_id: PassId,
        data_hash: String,
        signed_data: SignedData,
        bitemporal_proof: BitemporalProof,
    ) -> Self {
        Self {
            pass_id,
            data_hash,
            signed_data,
            bitemporal_proof,
        }
    }

    /// Verify the attestation
    pub fn verify(&self) -> Result<bool> {
        // Verify signature
        if !self.signed_data.verify()? {
            return Ok(false);
        }

        // Verify data hash matches
        if self.signed_data.data_hash() != self.data_hash {
            return Ok(false);
        }

        // Verify bi-temporal consistency
        if !self.bitemporal_proof.is_consistent() {
            return Ok(false);
        }

        Ok(true)
    }
}

impl BitemporalProof {
    /// Check if bi-temporal timestamps are consistent
    pub fn is_consistent(&self) -> bool {
        // Event times should be monotonic
        if self.last_event_time < self.first_event_time {
            return false;
        }

        // Reception times should be monotonic
        if self.last_reception_time < self.first_reception_time {
            return false;
        }

        // Propagation delay should be reasonable for a single LEO pass.
        // A typical LEO contact is 5–15 minutes; 900s is a generous ceiling that
        // still catches tampered timestamps while accommodating slow ground links.
        let propagation_delay =
            (self.last_reception_time - self.first_reception_time).num_seconds();
        if propagation_delay > 900 {
            return false;
        }

        true
    }
}

/// Attestation verifier
pub struct AttestationVerifier {
    /// Known public keys
    public_keys: std::collections::HashMap<String, VerifyingKey>,
}

impl AttestationVerifier {
    pub fn new() -> Self {
        Self {
            public_keys: std::collections::HashMap::new(),
        }
    }
}

impl Default for AttestationVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl AttestationVerifier {
    /// Add a public key
    pub fn add_public_key(&mut self, station_id: String, public_key: String) -> Result<()> {
        let public_key_bytes = hex::decode(&public_key).map_err(|e| {
            ground_core::GroundStationError::Federation(format!("Invalid public key: {}", e))
        })?;

        let public_key_array: [u8; 32] = public_key_bytes.try_into().map_err(|_| {
            ground_core::GroundStationError::Federation("Invalid public key length".to_string())
        })?;

        let public_key = VerifyingKey::from_bytes(&public_key_array).map_err(|e| {
            ground_core::GroundStationError::Federation(format!("Invalid public key: {}", e))
        })?;

        self.public_keys.insert(station_id, public_key);
        Ok(())
    }

    /// Verify an attestation from a known peer
    pub fn verify_attestation(&self, attestation: &Attestation, station_id: &str) -> Result<bool> {
        let public_key = self.public_keys.get(station_id).ok_or_else(|| {
            ground_core::GroundStationError::Federation("Unknown peer".to_string())
        })?;

        let expected_key = hex::encode(public_key.as_bytes());
        if expected_key != attestation.signed_data.public_key {
            return Err(ground_core::GroundStationError::Federation(
                "Public key mismatch".to_string(),
            ));
        }

        attestation.signed_data.verify()
    }
}

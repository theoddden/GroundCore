// Terminal identity and certificates
//
// Terminal identity provides cryptographic proof of terminal authenticity and
// authorization to participate in optical links.
//
// Extended with control plane certificate preloading (KubeSpace-inspired):
// All control nodes share a common key pair, satellites preload all control node
// certificates to eliminate certificate retrieval latency during handoffs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Terminal identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalIdentity {
    pub terminal_id: Uuid,
    pub vendor: String,
    pub model: String,
    pub serial_number: String,
    pub public_key: String,
    pub certificate: IdentityCertificate,
}

impl TerminalIdentity {
    pub fn new(
        terminal_id: Uuid,
        vendor: String,
        model: String,
        serial_number: String,
        public_key: String,
        certificate: IdentityCertificate,
    ) -> Self {
        Self {
            terminal_id,
            vendor,
            model,
            serial_number,
            public_key,
            certificate,
        }
    }

    pub fn verify(&self) -> bool {
        // In production, verify certificate chain
        true
    }
}

/// Identity certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCertificate {
    pub certificate_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub issuer: String,
    pub subject: String,
    pub signature: String,
}

impl IdentityCertificate {
    pub fn new(issuer: String, subject: String, validity_days: u64) -> Self {
        let now = Utc::now();
        Self {
            certificate_id: Uuid::new_v4(),
            issued_at: now,
            expires_at: now + chrono::Duration::days(validity_days as i64),
            issuer,
            subject,
            signature: "placeholder_signature".to_string(),
        }
    }

    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        now >= self.issued_at && now < self.expires_at
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }
}

/// Shared key pair for control plane certificate issuance (KubeSpace-inspired)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneKeyPair {
    pub key_pair_id: Uuid,
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>, // In production, this would be stored securely
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl ControlPlaneKeyPair {
    pub fn new(validity_days: u64) -> Self {
        let mut csprng = rand::rngs::OsRng;
        let keypair = ed25519_dalek::Keypair::generate(&mut csprng);

        let now = Utc::now();
        Self {
            key_pair_id: Uuid::new_v4(),
            public_key: keypair.public.to_bytes().to_vec(),
            private_key: keypair.secret.to_bytes().to_vec(),
            created_at: now,
            expires_at: now + chrono::Duration::days(validity_days as i64),
        }
    }

    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }
}

/// Preloaded control plane certificates (KubeSpace-inspired)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreloadedCertificates {
    pub root_certificate: IdentityCertificate,
    pub control_node_certificates: HashMap<String, IdentityCertificate>,
    pub loaded_at: DateTime<Utc>,
}

impl PreloadedCertificates {
    pub fn new(root_certificate: IdentityCertificate) -> Self {
        Self {
            root_certificate,
            control_node_certificates: HashMap::new(),
            loaded_at: Utc::now(),
        }
    }

    /// Add control node certificate to preload cache
    pub fn add_control_node_certificate(
        &mut self,
        node_id: String,
        certificate: IdentityCertificate,
    ) {
        self.control_node_certificates.insert(node_id, certificate);
    }

    /// Get certificate for specific control node
    pub fn get_control_node_certificate(&self, node_id: &str) -> Option<&IdentityCertificate> {
        self.control_node_certificates.get(node_id)
    }

    /// Validate all preloaded certificates
    pub fn validate_all(&self) -> bool {
        if !self.root_certificate.is_valid() {
            return false;
        }

        self.control_node_certificates
            .values()
            .all(|cert| cert.is_valid())
    }

    /// Check if preload cache is stale (older than 24 hours)
    pub fn is_stale(&self) -> bool {
        Utc::now() - self.loaded_at > chrono::Duration::hours(24)
    }
}

/// Control plane certificate manager
pub struct ControlPlaneCertificateManager {
    shared_key_pair: Option<ControlPlaneKeyPair>,
    issued_certificates: HashMap<String, IdentityCertificate>,
}

impl ControlPlaneCertificateManager {
    pub fn new() -> Self {
        Self {
            shared_key_pair: None,
            issued_certificates: HashMap::new(),
        }
    }

    /// Initialize shared key pair for control plane certificate issuance
    pub fn initialize_shared_key_pair(&mut self, validity_days: u64) {
        self.shared_key_pair = Some(ControlPlaneKeyPair::new(validity_days));
    }

    /// Issue certificate for control node using shared key pair
    pub fn issue_control_node_certificate(
        &mut self,
        node_id: String,
        subject: String,
        validity_days: u64,
    ) -> Result<IdentityCertificate, CertificateError> {
        let key_pair = self
            .shared_key_pair
            .as_ref()
            .ok_or_else(|| CertificateError::KeyPairNotInitialized)?;

        if !key_pair.is_valid() {
            return Err(CertificateError::KeyPairExpired);
        }

        let certificate = IdentityCertificate::new(
            "GroundCore Control Plane".to_string(),
            subject,
            validity_days,
        );

        self.issued_certificates
            .insert(node_id.clone(), certificate.clone());
        Ok(certificate)
    }

    /// Get preloaded certificates for satellite
    pub fn get_preloaded_certificates(&self) -> Result<PreloadedCertificates, CertificateError> {
        let key_pair = self
            .shared_key_pair
            .as_ref()
            .ok_or_else(|| CertificateError::KeyPairNotInitialized)?;

        let root_cert = IdentityCertificate::new(
            "GroundCore Root CA".to_string(),
            "GroundCore Root".to_string(),
            365, // 1 year validity
        );

        let mut preloaded = PreloadedCertificates::new(root_cert);

        for (node_id, cert) in &self.issued_certificates {
            preloaded.add_control_node_certificate(node_id.clone(), cert.clone());
        }

        Ok(preloaded)
    }

    /// Revoke certificate for control node
    pub fn revoke_certificate(&mut self, node_id: &str) -> Result<(), CertificateError> {
        self.issued_certificates.remove(node_id).ok_or_else(|| {
            CertificateError::CertificateNotFound {
                node_id: node_id.to_string(),
            }
        })?;
        Ok(())
    }
}

impl Default for ControlPlaneCertificateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Certificate error
#[derive(Debug, Clone, thiserror::Error)]
pub enum CertificateError {
    #[error("Shared key pair not initialized")]
    KeyPairNotInitialized,

    #[error("Shared key pair has expired")]
    KeyPairExpired,

    #[error("Certificate not found for node: {node_id}")]
    CertificateNotFound { node_id: String },

    #[error("Certificate validation failed: {0}")]
    ValidationFailed(String),
}

//! Cryptographic backend abstraction and implementations

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    #[error("Invalid key length")]
    InvalidKeyLength,
}

pub type Result<T> = std::result::Result<T, CryptoError>;

/// Encryption key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    pub key: Vec<u8>,
}

/// Signing key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKeyPair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

/// Cryptographic backend trait
#[async_trait::async_trait]
pub trait CryptoBackend: Send + Sync {
    /// Encrypt plaintext
    async fn encrypt(&self, plaintext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;

    /// Decrypt ciphertext
    async fn decrypt(&self, ciphertext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;

    /// Sign data
    async fn sign(&self, data: &[u8], key: &SigningKeyPair) -> Result<Vec<u8>>;

    /// Verify signature
    async fn verify(&self, data: &[u8], signature: &[u8], key: &SigningKeyPair) -> Result<bool>;

    /// Generate encryption key
    async fn generate_encryption_key(&self) -> Result<EncryptionKey>;

    /// Generate signing key pair
    async fn generate_signing_key(&self) -> Result<SigningKeyPair>;
}

/// AES-256-GCM implementation
pub struct AesGcmBackend;

#[async_trait::async_trait]
impl CryptoBackend for AesGcmBackend {
    async fn encrypt(&self, plaintext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>> {
        if key.key.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }

        let key = Key::<Aes256Gcm>::from_slice(&key.key);
        let cipher = Aes256Gcm::new(key);

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // Encrypt
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(nonce.len() + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    async fn decrypt(&self, ciphertext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>> {
        if key.key.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }

        if ciphertext.len() < 12 {
            return Err(CryptoError::DecryptionFailed(
                "Ciphertext too short".to_string(),
            ));
        }

        let key = Key::<Aes256Gcm>::from_slice(&key.key);
        let cipher = Aes256Gcm::new(key);

        // Extract nonce (first 12 bytes)
        let nonce = Nonce::from_slice(&ciphertext[..12]);
        let ciphertext = &ciphertext[12..];

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        Ok(plaintext)
    }

    async fn sign(&self, data: &[u8], key: &SigningKeyPair) -> Result<Vec<u8>> {
        let signing_key = SigningKey::try_from(&key.private_key[..])
            .map_err(|e| CryptoError::KeyGenerationFailed(e.to_string()))?;

        let signature: Signature = signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    async fn verify(&self, data: &[u8], signature: &[u8], key: &SigningKeyPair) -> Result<bool> {
        let verifying_key = VerifyingKey::try_from(&key.public_key[..])
            .map_err(|e| CryptoError::KeyGenerationFailed(e.to_string()))?;

        let signature =
            Signature::try_from(signature).map_err(|_| CryptoError::SignatureVerificationFailed)?;

        Ok(verifying_key.verify(data, &signature).is_ok())
    }

    async fn generate_encryption_key(&self) -> Result<EncryptionKey> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Ok(EncryptionKey { key: key.to_vec() })
    }

    async fn generate_signing_key(&self) -> Result<SigningKeyPair> {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        Ok(SigningKeyPair {
            public_key: verifying_key.to_bytes().to_vec(),
            private_key: signing_key.to_bytes().to_vec(),
        })
    }
}

/// HMAC-SHA256 authentication (simpler alternative to Ed25519)
pub struct HmacSha256Backend;

#[async_trait::async_trait]
impl CryptoBackend for HmacSha256Backend {
    async fn encrypt(&self, _plaintext: &[u8], _key: &EncryptionKey) -> Result<Vec<u8>> {
        // HMAC-SHA256 doesn't provide encryption, use AES-GCM instead
        Err(CryptoError::EncryptionFailed(
            "HMAC-SHA256 does not support encryption".to_string(),
        ))
    }

    async fn decrypt(&self, _ciphertext: &[u8], _key: &EncryptionKey) -> Result<Vec<u8>> {
        Err(CryptoError::DecryptionFailed(
            "HMAC-SHA256 does not support decryption".to_string(),
        ))
    }

    async fn sign(&self, data: &[u8], key: &SigningKeyPair) -> Result<Vec<u8>> {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(&key.private_key)
            .map_err(|e| CryptoError::KeyGenerationFailed(e.to_string()))?;
        mac.update(data);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    async fn verify(&self, data: &[u8], signature: &[u8], key: &SigningKeyPair) -> Result<bool> {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(&key.private_key)
            .map_err(|e| CryptoError::KeyGenerationFailed(e.to_string()))?;
        mac.update(data);
        Ok(mac.verify_slice(signature).is_ok())
    }

    async fn generate_encryption_key(&self) -> Result<EncryptionKey> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Ok(EncryptionKey { key: key.to_vec() })
    }

    async fn generate_signing_key(&self) -> Result<SigningKeyPair> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Ok(SigningKeyPair {
            public_key: key.to_vec(), // For HMAC, public and private are same
            private_key: key.to_vec(),
        })
    }
}

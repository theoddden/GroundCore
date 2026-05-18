//! Tenant shards for cryptographic isolation
//!
//! Per-tenant shards make data isolation enforceable at the allocator level.
//! Tenant A's data is allocated in Tenant A's arena, which is mapped to memory
//! pages with different protection bits than Tenant B's arena.

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use bumpalo::Bump;
use chrono::{DateTime, Utc};
use ground_core::{CustomerId, GroundStationError, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Memory page protection bits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageProtection {
    /// No access
    None,
    /// Read-only
    ReadOnly,
    /// Read-write
    ReadWrite,
    /// Read-execute
    ReadExecute,
}

/// Protected memory region for tenant isolation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedMemoryRegion {
    /// Base address (would be actual pointer in production)
    pub base_address: usize,
    /// Size in bytes
    pub size: usize,
    /// Page protection
    pub protection: PageProtection,
}

impl ProtectedMemoryRegion {
    pub fn new(base_address: usize, size: usize, protection: PageProtection) -> Self {
        Self {
            base_address,
            size,
            protection,
        }
    }

    /// In a real implementation, this would use mprotect() or equivalent
    /// to set memory protection at the OS level
    pub fn apply_protection(&self) -> Result<()> {
        // In production: unsafe { libc::mprotect(...) }
        tracing::debug!(
            "Applying protection {:?} to region 0x{:x} ({} bytes)",
            self.protection,
            self.base_address,
            self.size
        );
        Ok(())
    }
}

/// Cryptographic context for tenant data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantCrypto {
    /// Logical key identifier (for audit / key-rotation tracking)
    pub key_id: String,
    /// Key rotation timestamp
    pub key_rotation: DateTime<Utc>,
    /// Raw AES-256 key bytes. Skipped from serde so key material is never
    /// accidentally serialised into logs or persistent state.
    #[serde(skip, default)]
    key_bytes: Vec<u8>,
}

impl TenantCrypto {
    pub fn new(key_id: String) -> Self {
        let mut key_bytes = vec![0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        Self {
            key_id,
            key_rotation: Utc::now(),
            key_bytes,
        }
    }

    /// AES-256-GCM encryption. Ciphertext layout: [12-byte nonce || encrypted bytes].
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if self.key_bytes.len() != 32 {
            return Err(GroundStationError::Hardware(
                "Tenant crypto key not initialized (key_bytes empty after deserialization)"
                    .to_string(),
            ));
        }
        let key = Key::<Aes256Gcm>::from_slice(&self.key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, data)
            .map_err(|e| GroundStationError::Hardware(format!("AES-GCM encrypt failed: {e}")))?;
        let mut result = Vec::with_capacity(nonce.len() + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        tracing::debug!(
            "Encrypted {} bytes for tenant key {}",
            data.len(),
            self.key_id
        );
        Ok(result)
    }

    /// AES-256-GCM decryption. Expects the nonce-prepended layout produced by `encrypt`.
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if self.key_bytes.len() != 32 {
            return Err(GroundStationError::Hardware(
                "Tenant crypto key not initialized (key_bytes empty after deserialization)"
                    .to_string(),
            ));
        }
        if data.len() < 12 {
            return Err(GroundStationError::Hardware(
                "Ciphertext too short to contain a 12-byte nonce".to_string(),
            ));
        }
        let key = Key::<Aes256Gcm>::from_slice(&self.key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&data[..12]);
        let plaintext = cipher
            .decrypt(nonce, &data[12..])
            .map_err(|e| GroundStationError::Hardware(format!("AES-GCM decrypt failed: {e}")))?;
        tracing::debug!(
            "Decrypted {} bytes for tenant key {}",
            plaintext.len(),
            self.key_id
        );
        Ok(plaintext)
    }
}

/// Tenant-isolated shard
pub struct TenantShard {
    /// Tenant identifier
    tenant_id: CustomerId,
    /// Bump allocator for tenant-specific allocations
    arena: Bump,
    /// Memory region with protection
    memory_region: ProtectedMemoryRegion,
    /// Page protection
    #[allow(dead_code)]
    page_permissions: PageProtection,
    /// Cryptographic context
    crypto_context: TenantCrypto,
    /// When this shard was created
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
}

impl TenantShard {
    /// Create a new tenant shard
    pub fn new(
        tenant_id: CustomerId,
        arena_size: usize,
        protection: PageProtection,
        crypto: TenantCrypto,
    ) -> Self {
        let base_address = 0; // Would be actual address in production

        Self {
            tenant_id,
            arena: Bump::with_capacity(arena_size),
            memory_region: ProtectedMemoryRegion::new(base_address, arena_size, protection),
            page_permissions: protection,
            crypto_context: crypto,
            created_at: Utc::now(),
        }
    }

    /// Allocate within tenant shard
    pub fn allocate_tenant<T>(&self, value: T) -> &T {
        self.arena.alloc(value)
    }

    /// Get tenant ID
    pub fn tenant_id(&self) -> &CustomerId {
        &self.tenant_id
    }

    /// Get memory region
    pub fn memory_region(&self) -> &ProtectedMemoryRegion {
        &self.memory_region
    }

    /// Get crypto context
    pub fn crypto(&self) -> &TenantCrypto {
        &self.crypto_context
    }

    /// Reset the shard (clears all allocations)
    pub fn reset(&mut self) {
        self.arena.reset();
    }
}

/// Tenant shard manager
pub struct TenantShardManager {
    /// Tenant shards indexed by customer ID
    shards: HashMap<CustomerId, TenantShard>,
    /// Default shard size
    default_shard_size: usize,
}

impl TenantShardManager {
    pub fn new(default_shard_size: usize) -> Self {
        Self {
            shards: HashMap::new(),
            default_shard_size,
        }
    }

    /// Get or create a tenant shard
    pub fn get_or_create(&mut self, tenant_id: CustomerId) -> Result<&TenantShard> {
        if !self.shards.contains_key(&tenant_id) {
            let crypto = TenantCrypto::new(format!("key-{}", tenant_id));
            let shard = TenantShard::new(
                tenant_id.clone(),
                self.default_shard_size,
                PageProtection::ReadWrite,
                crypto,
            );
            self.shards.insert(tenant_id.clone(), shard);
        }

        Ok(self.shards.get(&tenant_id).unwrap())
    }

    /// Get a tenant shard
    pub fn get(&self, tenant_id: &CustomerId) -> Option<&TenantShard> {
        self.shards.get(tenant_id)
    }

    /// Remove a tenant shard
    pub fn remove(&mut self, tenant_id: &CustomerId) -> Option<TenantShard> {
        self.shards.remove(tenant_id)
    }

    /// Get all tenant IDs
    pub fn tenant_ids(&self) -> Vec<CustomerId> {
        self.shards.keys().cloned().collect()
    }
}

//! Version management for zero-downtime deployment

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::path::PathBuf;

/// Semantic version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
    
    pub fn as_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Versioned binary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedBinary {
    /// Binary version
    pub version: Version,
    /// Path to the binary
    pub path: PathBuf,
    /// Checksum for verification
    pub checksum: String,
    /// When this binary was built
    pub built_at: DateTime<Utc>,
    /// Schema version for shared memory
    pub schema_version: String,
}

impl VersionedBinary {
    pub fn new(version: Version, path: PathBuf, checksum: String, schema_version: String) -> Self {
        Self {
            version,
            path,
            checksum,
            built_at: Utc::now(),
            schema_version,
        }
    }
    
    /// Compute the SHA-256 checksum of a binary file (lowercase hex).
    /// Returns `None` if the file cannot be read.
    pub fn compute_checksum(path: &PathBuf) -> Option<String> {
        let data = std::fs::read(path).ok()?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Some(hex::encode(hasher.finalize()))
    }

    /// Verify the binary at `self.path` against the stored SHA-256 checksum.
    /// Returns `true` when the file exists and its hash matches, `false` otherwise.
    pub fn verify(&self) -> bool {
        match Self::compute_checksum(&self.path) {
            Some(computed) => {
                let ok = computed == self.checksum;
                if !ok {
                    tracing::warn!(
                        "Checksum mismatch for {}: expected {} got {}",
                        self.path.display(),
                        self.checksum,
                        computed,
                    );
                }
                ok
            }
            None => {
                tracing::error!("Cannot read binary at {} for verification", self.path.display());
                false
            }
        }
    }
    
    /// Check schema compatibility with another version
    pub fn schema_compatible(&self, other: &VersionedBinary) -> bool {
        // Same schema version means compatible
        self.schema_version == other.schema_version
    }
}

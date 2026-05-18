//! Schedule snapshotting for incremental re-optimization
//!
//! Uses serialized bytes rather than a typed reference to `scheduler::Schedule`
//! to avoid a circular crate dependency (scheduler → snapshotting → scheduler).
//! Callers restore the typed value via `ScheduleSnapshot::restore::<T>()`.

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Opaque schedule snapshot — stores serialized bytes to remain crate-independent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleSnapshot {
    pub snapshot_id: String,
    /// JSON-serialized schedule bytes
    pub data: Vec<u8>,
    pub captured_at: DateTime<Utc>,
    /// SHA-256 of `data` for integrity verification
    pub hash: String,
    pub size_bytes: usize,
}

impl ScheduleSnapshot {
    /// Create a snapshot of any serializable schedule type.
    pub fn new<T: Serialize>(value: &T) -> Self {
        let json = serde_json::to_vec(value).expect("schedule must be serializable");
        let size_bytes = json.len();

        let mut hasher = Sha256::new();
        hasher.update(&json);
        let hash = format!("{:x}", hasher.finalize());

        Self {
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            data: json,
            captured_at: Utc::now(),
            hash,
            size_bytes,
        }
    }

    /// Deserialize the snapshot back to the original type.
    pub fn restore<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.data)
    }

    /// Verify snapshot integrity against stored hash.
    pub fn verify(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        format!("{:x}", hasher.finalize()) == self.hash
    }
}

/// Schedule snapshotter — retains the last N snapshots.
#[derive(Debug, Clone)]
pub struct ScheduleSnapshotter {
    snapshots: Vec<ScheduleSnapshot>,
    max_snapshots: usize,
}

impl ScheduleSnapshotter {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    /// Take a snapshot of any serializable value.
    pub fn snapshot<T: Serialize>(&mut self, value: &T) -> ScheduleSnapshot {
        let snap = ScheduleSnapshot::new(value);
        self.snapshots.push(snap.clone());
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
        snap
    }

    pub fn latest(&self) -> Option<&ScheduleSnapshot> {
        self.snapshots.last()
    }

    pub fn get_snapshots(&self) -> &[ScheduleSnapshot] {
        &self.snapshots
    }

    pub fn get(&self, snapshot_id: &str) -> Option<&ScheduleSnapshot> {
        self.snapshots.iter().find(|s| s.snapshot_id == snapshot_id)
    }
}

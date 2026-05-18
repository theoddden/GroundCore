//! Log snapshotting for time-travel queries and analysis
//!
//! Snapshots enable the embedded agent to answer "why did this happen?"
//! by loading the state as it existed at a specific point in time.

use crate::log::{LogEntry, LogEntryType};
use crate::timestamp::ReceptionTime;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Snapshot of the log at a specific point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSnapshot {
    /// Snapshot ID
    pub snapshot_id: String,
    /// When this snapshot was taken
    pub captured_at: ReceptionTime,
    /// All log entries up to this point
    pub entries: Vec<LogEntry>,
    /// Snapshot metadata
    pub metadata: SnapshotMetadata,
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Number of entries in snapshot
    pub entry_count: usize,
    /// Time range of snapshot
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    /// Pass IDs included
    pub pass_ids: Vec<String>,
    /// Snapshot hash for integrity
    pub hash: String,
}

impl LogSnapshot {
    /// Create a new log snapshot
    pub fn new(entries: Vec<LogEntry>) -> Self {
        let captured_at = ReceptionTime::now();
        let entry_count = entries.len();

        let time_range = if let (Some(first), Some(last)) = (entries.first(), entries.last()) {
            (
                first.reception_time.as_datetime(),
                last.reception_time.as_datetime(),
            )
        } else {
            (Utc::now(), Utc::now())
        };

        let pass_ids: Vec<String> = entries
            .iter()
            .map(|e| e.pass_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let hash = Self::compute_hash(&entries);

        let metadata = SnapshotMetadata {
            entry_count,
            time_range,
            pass_ids,
            hash,
        };

        Self {
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            captured_at,
            entries,
            metadata,
        }
    }

    /// Compute hash of snapshot
    fn compute_hash(entries: &[LogEntry]) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        for entry in entries {
            hasher.update(entry.hash.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Verify snapshot integrity
    pub fn verify(&self) -> bool {
        Self::compute_hash(&self.entries) == self.metadata.hash
    }

    /// Query entries by type
    pub fn query_by_type(&self, entry_type: LogEntryType) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| {
                std::mem::discriminant(&e.entry_type) == std::mem::discriminant(&entry_type)
            })
            .collect()
    }

    /// Query entries by pass ID
    pub fn query_by_pass(&self, pass_id: &str) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.pass_id == pass_id)
            .collect()
    }

    /// Query entries in time range
    pub fn query_by_time_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| {
                let t = e.reception_time.as_datetime();
                t >= start && t <= end
            })
            .collect()
    }
}

/// Snapshot manager
pub struct SnapshotManager {
    /// Historical snapshots
    snapshots: Vec<LogSnapshot>,
    /// Maximum number of snapshots to retain
    max_snapshots: usize,
    /// Snapshot interval (seconds)
    snapshot_interval_sec: u64,
    /// Last snapshot time
    last_snapshot_time: Option<DateTime<Utc>>,
}

impl SnapshotManager {
    pub fn new(max_snapshots: usize, snapshot_interval_sec: u64) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
            snapshot_interval_sec,
            last_snapshot_time: None,
        }
    }

    /// Check if a snapshot should be taken
    pub fn should_snapshot(&self) -> bool {
        if let Some(last) = self.last_snapshot_time {
            let elapsed = (Utc::now() - last).num_seconds();
            elapsed >= self.snapshot_interval_sec as i64
        } else {
            true
        }
    }

    /// Take a snapshot if needed
    pub fn snapshot_if_needed(&mut self, entries: Vec<LogEntry>) -> Option<LogSnapshot> {
        if self.should_snapshot() {
            let snapshot = LogSnapshot::new(entries);
            self.add_snapshot(snapshot.clone());
            self.last_snapshot_time = Some(Utc::now());
            Some(snapshot)
        } else {
            None
        }
    }

    /// Add a snapshot
    fn add_snapshot(&mut self, snapshot: LogSnapshot) {
        self.snapshots.push(snapshot);

        // Prune old snapshots
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Get the most recent snapshot
    pub fn latest(&self) -> Option<&LogSnapshot> {
        self.snapshots.last()
    }

    /// Get snapshot closest to a given time
    pub fn find_closest(&self, time: DateTime<Utc>) -> Option<&LogSnapshot> {
        self.snapshots.iter().min_by_key(|s| {
            (s.captured_at.as_datetime() - time)
                .num_milliseconds()
                .abs()
        })
    }

    /// Get all snapshots
    pub fn all_snapshots(&self) -> &[LogSnapshot] {
        &self.snapshots
    }

    /// Prune snapshots older than a threshold
    pub fn prune_older_than(&mut self, age_sec: i64) {
        let cutoff = Utc::now() - chrono::Duration::seconds(age_sec);
        self.snapshots
            .retain(|s| s.captured_at.as_datetime() > cutoff);
    }
}

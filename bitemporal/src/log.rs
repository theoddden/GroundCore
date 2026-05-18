//! Bi-temporal log implementation

use crate::timestamp::{EventTime, ReceptionTime};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Log entry with bi-temporal timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique entry identifier
    pub entry_id: String,
    /// Pass ID this entry belongs to
    pub pass_id: PassId,
    /// Entry type
    pub entry_type: LogEntryType,
    /// Event time
    pub event_time: EventTime,
    /// Reception time
    pub reception_time: ReceptionTime,
    /// Entry data (serialized)
    pub data: serde_json::Value,
    /// Hash of this entry for integrity verification
    pub hash: String,
    /// Previous entry hash (for chain integrity)
    pub previous_hash: Option<String>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(
        pass_id: PassId,
        entry_type: LogEntryType,
        event_time: EventTime,
        reception_time: ReceptionTime,
        data: serde_json::Value,
        previous_hash: Option<String>,
    ) -> Self {
        let entry_id = uuid::Uuid::new_v4().to_string();
        let hash = Self::compute_hash(
            &entry_id,
            &pass_id,
            &entry_type,
            &event_time,
            &reception_time,
            &data,
            &previous_hash,
        );

        Self {
            entry_id,
            pass_id,
            entry_type,
            event_time,
            reception_time,
            data,
            hash,
            previous_hash,
        }
    }

    /// Compute hash of log entry
    fn compute_hash(
        entry_id: &str,
        pass_id: &PassId,
        entry_type: &LogEntryType,
        event_time: &EventTime,
        reception_time: &ReceptionTime,
        data: &serde_json::Value,
        previous_hash: &Option<String>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(entry_id.as_bytes());
        hasher.update(pass_id.as_bytes());
        hasher.update(serde_json::to_string(entry_type).unwrap().as_bytes());
        hasher.update(serde_json::to_string(event_time).unwrap().as_bytes());
        hasher.update(serde_json::to_string(reception_time).unwrap().as_bytes());
        hasher.update(serde_json::to_string(data).unwrap().as_bytes());
        if let Some(prev) = previous_hash {
            hasher.update(prev.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Verify the integrity of this log entry
    pub fn verify(&self) -> bool {
        let computed_hash = Self::compute_hash(
            &self.entry_id,
            &self.pass_id,
            &self.entry_type,
            &self.event_time,
            &self.reception_time,
            &self.data,
            &self.previous_hash,
        );
        computed_hash == self.hash
    }
}

/// Types of log entries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEntryType {
    /// Sample received from SDR
    SampleReceived {
        sample_id: u64,
        frequency: u64,
        snr: f32,
    },
    /// Demodulator state change
    DemodulatorStateChange {
        carrier_locked: bool,
        symbol_sync: bool,
    },
    /// Hardware allocation
    HardwareAllocation {
        device_id: String,
        hardware_type: String,
    },
    /// Hardware release
    HardwareRelease { device_id: String },
    /// Pass started
    PassStarted {
        satellite_id: String,
        expected_duration_sec: u64,
    },
    /// Pass completed
    PassCompleted {
        samples_received: u64,
        bytes_decoded: u64,
    },
    /// Pass failed
    PassFailed { reason: String },
    /// Shadow SDR promoted
    ShadowPromoted {
        from_device: String,
        to_device: String,
    },
    /// Scheduler decision
    SchedulerDecision {
        decision_type: String,
        rationale: String,
    },
    /// Federation event
    FederationEvent {
        peer_station: String,
        event_type: String,
    },
    /// Error occurred
    Error { error_type: String, message: String },
}

/// Bi-temporal log
pub struct BitemporalLog {
    /// Log entries indexed by entry ID
    entries: HashMap<String, LogEntry>,
    /// Entries indexed by pass ID
    pass_entries: HashMap<PassId, Vec<String>>,
    /// Latest hash for chain integrity
    latest_hash: Option<String>,
}

impl BitemporalLog {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            pass_entries: HashMap::new(),
            latest_hash: None,
        }
    }

    /// Append a log entry
    pub fn append(&mut self, entry: LogEntry) -> Result<()> {
        // Verify entry integrity
        if !entry.verify() {
            return Err(ground_core::GroundStationError::Database(
                "Log entry verification failed".to_string(),
            ));
        }

        // Verify chain integrity
        if let Some(latest) = &self.latest_hash
            && entry.previous_hash.as_ref() != Some(latest)
        {
            return Err(ground_core::GroundStationError::Database(
                "Log chain integrity broken".to_string(),
            ));
        }

        let entry_id = entry.entry_id.clone();
        let pass_id = entry.pass_id.clone();
        let entry_hash = entry.hash.clone();

        self.entries.insert(entry_id.clone(), entry);
        self.pass_entries
            .entry(pass_id)
            .or_default()
            .push(entry_id);
        self.latest_hash = Some(entry_hash);

        Ok(())
    }

    /// Get a log entry by ID
    pub fn get(&self, entry_id: &str) -> Option<&LogEntry> {
        self.entries.get(entry_id)
    }

    /// Get all entries for a pass
    pub fn get_pass_entries(&self, pass_id: &PassId) -> Vec<&LogEntry> {
        self.pass_entries
            .get(pass_id)
            .map(|ids| ids.iter().filter_map(|id| self.entries.get(id)).collect())
            .unwrap_or_default()
    }

    /// Verify the integrity of the entire log chain
    pub fn verify_chain(&self) -> bool {
        let mut previous_hash: Option<String> = None;

        // Sort entries by reception time for chain verification
        let mut sorted_entries: Vec<_> = self.entries.values().collect();
        sorted_entries.sort_by_key(|e| e.reception_time.as_datetime());

        for entry in sorted_entries {
            if entry.previous_hash != previous_hash {
                return false;
            }
            if !entry.verify() {
                return false;
            }
            previous_hash = Some(entry.hash.clone());
        }

        true
    }

    /// Get log statistics
    pub fn stats(&self) -> LogStats {
        LogStats {
            total_entries: self.entries.len(),
            total_passes: self.pass_entries.len(),
            latest_hash: self.latest_hash.clone(),
            chain_valid: self.verify_chain(),
        }
    }
}

impl Default for BitemporalLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Log statistics
#[derive(Debug, Clone)]
pub struct LogStats {
    pub total_entries: usize,
    pub total_passes: usize,
    pub latest_hash: Option<String>,
    pub chain_valid: bool,
}

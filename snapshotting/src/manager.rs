//! Central snapshot manager

use crate::demodulator::DemodulatorSnapshotter;
use crate::federation::FederationSnapshotter;
use crate::schedule::ScheduleSnapshotter;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot type
    pub snapshot_type: SnapshotType,
    /// Snapshot ID
    pub snapshot_id: String,
    /// When captured
    pub captured_at: DateTime<Utc>,
    /// Size in bytes
    pub size_bytes: usize,
}

/// Snapshot type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotType {
    Schedule,
    Demodulator,
    Federation,
}

/// Central snapshot manager
#[derive(Debug, Clone)]
pub struct SnapshotManager {
    schedule_snapshotter: ScheduleSnapshotter,
    demodulator_snapshotter: DemodulatorSnapshotter,
    federation_snapshotter: FederationSnapshotter,
}

impl SnapshotManager {
    pub fn new(max_schedule_snapshots: usize) -> Self {
        Self {
            schedule_snapshotter: ScheduleSnapshotter::new(max_schedule_snapshots),
            demodulator_snapshotter: DemodulatorSnapshotter::new(),
            federation_snapshotter: FederationSnapshotter::new(),
        }
    }
    
    /// Get schedule snapshotter
    pub fn schedule(&mut self) -> &mut ScheduleSnapshotter {
        &mut self.schedule_snapshotter
    }
    
    /// Get demodulator snapshotter
    pub fn demodulator(&mut self) -> &mut DemodulatorSnapshotter {
        &mut self.demodulator_snapshotter
    }
    
    /// Get federation snapshotter
    pub fn federation(&mut self) -> &mut FederationSnapshotter {
        &mut self.federation_snapshotter
    }
    
    /// Get all snapshot metadata
    pub fn all_metadata(&self) -> Vec<SnapshotMetadata> {
        let mut metadata = Vec::new();
        
        // Schedule snapshots
        for snapshot in self.schedule_snapshotter.get_snapshots() {
            metadata.push(SnapshotMetadata {
                snapshot_type: SnapshotType::Schedule,
                snapshot_id: snapshot.snapshot_id.clone(),
                captured_at: snapshot.captured_at,
                size_bytes: snapshot.size_bytes,
            });
        }
        
        // Demodulator snapshots
        for snapshot in self.demodulator_snapshotter.get_snapshots() {
            let size = serde_json::to_string(&snapshot).unwrap().len();
            metadata.push(SnapshotMetadata {
                snapshot_type: SnapshotType::Demodulator,
                snapshot_id: snapshot.snapshot_id.clone(),
                captured_at: snapshot.captured_at,
                size_bytes: size,
            });
        }
        
        // Federation snapshots
        for snapshot in self.federation_snapshotter.get_snapshots() {
            let size = serde_json::to_string(&snapshot).unwrap().len();
            metadata.push(SnapshotMetadata {
                snapshot_type: SnapshotType::Federation,
                snapshot_id: snapshot.snapshot_id.clone(),
                captured_at: snapshot.captured_at,
                size_bytes: size,
            });
        }
        
        metadata
    }
}

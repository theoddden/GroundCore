//! Demodulator state snapshotting for failover recovery
//!
//! Stores serialized bytes rather than a typed `rf_layer::DemodulatorSnapshot`
//! to avoid the circular crate dependency (rf-layer → snapshotting → rf-layer).
//! Callers restore the typed value via `DemodulatorSnapshotWrapper::restore::<T>()`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Opaque demodulator snapshot — crate-independent serialized bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemodulatorSnapshotWrapper {
    pub snapshot_id: String,
    pub pass_id: String,
    /// JSON-serialized demodulator state
    pub data: Vec<u8>,
    pub captured_at: DateTime<Utc>,
}

impl DemodulatorSnapshotWrapper {
    pub fn restore<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.data)
    }
}

/// Demodulator snapshotter
#[derive(Debug, Clone)]
pub struct DemodulatorSnapshotter {
    snapshots: Vec<DemodulatorSnapshotWrapper>,
}

impl DemodulatorSnapshotter {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    /// Take a snapshot of any serializable demodulator state.
    pub fn snapshot<T: Serialize>(
        &mut self,
        pass_id: String,
        state: &T,
    ) -> DemodulatorSnapshotWrapper {
        let wrapper = DemodulatorSnapshotWrapper {
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            pass_id,
            data: serde_json::to_vec(state).expect("demodulator state must be serializable"),
            captured_at: Utc::now(),
        };
        self.snapshots.push(wrapper.clone());
        wrapper
    }

    pub fn get_for_pass(&self, pass_id: &str) -> Option<&DemodulatorSnapshotWrapper> {
        self.snapshots.iter().find(|s| s.pass_id == pass_id)
    }

    pub fn get_snapshots(&self) -> &[DemodulatorSnapshotWrapper] {
        &self.snapshots
    }

    pub fn remove_pass(&mut self, pass_id: &str) {
        self.snapshots.retain(|s| s.pass_id != pass_id);
    }
}

//! Counterfactual reconstruction via bi-temporal snapshots
//!
//! This is the forensic feature Aalyria structurally can't match. Because every tick
//! snapshots bi-temporally, you can answer "what did the system believe at 14:03:02,
//! and when did belief first diverge from reality." Their planning engine can't replay
//! what-we-knew-when because it isn't built on a bi-temporal substrate. Defense buyers,
//! insurers, and regulators pay for forensics. This is your unfair advantage finally
//! weaponized.

use crate::divergence::MetricDivergence;
use bevy_ecs::world::World;
use chrono::{DateTime, Utc};
use ground_core::types::LinkId;
use std::collections::HashMap;

/// Twin state snapshot at a specific point in time
///
/// This captures the complete state of the digital twin at a given tick, including
/// the ECS world state and the divergence log. Because every tick is bi-temporally
/// stamped, you can replay exactly what the system believed at any moment.
#[derive(Debug)]
pub struct TwinSnapshot {
    /// When this snapshot was taken
    pub timestamp: DateTime<Utc>,
    /// ECS world state (serialized)
    ///
    /// Note: In practice, World is not directly serializable. This would be a
    /// serialized representation of the entity-component state.
    pub world_state: World,
    /// Divergence log at this point in time
    pub divergence_log: Vec<MetricDivergence>,
}

impl TwinSnapshot {
    /// Create a new twin snapshot
    pub fn new(timestamp: DateTime<Utc>, world_state: World, divergence_log: Vec<MetricDivergence>) -> Self {
        Self {
            timestamp,
            world_state,
            divergence_log,
        }
    }

    /// Get divergences that were active at this snapshot time
    pub fn active_divergences(&self) -> Vec<&MetricDivergence> {
        self.divergence_log
            .iter()
            .filter(|d| d.is_anomaly())
            .collect()
    }

    /// Get divergences that first appeared at or before this snapshot
    pub fn divergences_up_to(&self, time: DateTime<Utc>) -> Vec<&MetricDivergence> {
        self.divergence_log
            .iter()
            .filter(|d| d.bitemporal_stamp.reception_time.as_datetime() <= time)
            .collect()
    }
}

/// Snapshot manager for storing and retrieving twin state
///
/// This manages the lifecycle of twin snapshots, enabling counterfactual reconstruction
/// and forensic analysis.
pub struct SnapshotManager {
    /// Stored snapshots indexed by timestamp
    snapshots: HashMap<DateTime<Utc>, TwinSnapshot>,
    /// Maximum number of snapshots to retain
    max_snapshots: usize,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: HashMap::new(),
            max_snapshots,
        }
    }

    /// Store a snapshot
    pub fn store_snapshot(&mut self, snapshot: TwinSnapshot) {
        let timestamp = snapshot.timestamp;

        // Add the snapshot
        self.snapshots.insert(timestamp, snapshot);

        // Enforce max snapshot limit (remove oldest)
        if self.snapshots.len() > self.max_snapshots {
            let oldest_timestamp = self.snapshots.keys().min().copied().unwrap();
            self.snapshots.remove(&oldest_timestamp);
        }
    }

    /// Retrieve a snapshot by timestamp
    pub fn get_snapshot(&self, timestamp: DateTime<Utc>) -> Option<&TwinSnapshot> {
        self.snapshots.get(&timestamp)
    }

    /// Get the snapshot closest to a given timestamp
    pub fn get_closest_snapshot(&self, timestamp: DateTime<Utc>) -> Option<&TwinSnapshot> {
        if self.snapshots.is_empty() {
            return None;
        }

        let closest_timestamp = self
            .snapshots
            .keys()
            .min_by_key(|t| (**t - timestamp).abs().num_seconds().abs())
            .copied()?;

        self.snapshots.get(&closest_timestamp)
    }

    /// Get all snapshots in a time range
    pub fn get_snapshots_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&TwinSnapshot> {
        let mut snapshots: Vec<_> = self
            .snapshots
            .iter()
            .filter(|(t, _)| **t >= start && **t <= end)
            .map(|(_, s)| s)
            .collect();

        snapshots.sort_by_key(|s| s.timestamp);
        snapshots
    }

    /// Get the most recent snapshot
    pub fn latest_snapshot(&self) -> Option<&TwinSnapshot> {
        self.snapshots
            .values()
            .max_by_key(|s| s.timestamp)
    }

    /// Clear all snapshots
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Get the number of stored snapshots
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new(300) // Default: retain 5 minutes of snapshots (at 1-second tick)
    }
}

/// Counterfactual query result
///
/// This represents the result of a counterfactual query: "what would the system
/// have believed at time T if we hadn't observed divergence D?"
#[derive(Debug)]
pub struct CounterfactualQuery {
    /// Query timestamp
    pub query_time: DateTime<Utc>,
    /// What the system actually believed
    pub actual_belief: TwinSnapshot,
    /// What the system would have believed without the divergence
    pub counterfactual_belief: Option<TwinSnapshot>,
    /// Divergences that affected the belief
    pub affecting_divergences: Vec<MetricDivergence>,
}

impl SnapshotManager {
    /// Perform a counterfactual query
    ///
    /// Answer: "what did the system believe at time T, and when did belief first
    /// diverge from reality?"
    pub fn counterfactual_query(&self, query_time: DateTime<Utc>) -> CounterfactualQuery {
        let actual_belief = if let Some(snapshot) = self.get_closest_snapshot(query_time) {
            // Create a new snapshot with the same data (World is not Clone, so we use a placeholder)
            TwinSnapshot::new(
                snapshot.timestamp,
                World::new(),
                snapshot.divergence_log.clone(),
            )
        } else {
            TwinSnapshot::new(
                query_time,
                World::new(),
                Vec::new(),
            )
        };

        // Find divergences that first appeared before or at the query time
        let affecting_divergences: Vec<_> = actual_belief
            .divergence_log
            .iter()
            .filter(|d| {
                d.bitemporal_stamp.reception_time.as_datetime() <= query_time
                    || d.change_point_detected_at.map_or(false, |cp| cp <= query_time)
            })
            .cloned()
            .collect();

        // In a full implementation, we would reconstruct the counterfactual belief
        // by replaying the simulation without the affecting divergences.
        // For now, this is a placeholder.
        let counterfactual_belief = None;

        CounterfactualQuery {
            query_time,
            actual_belief,
            counterfactual_belief,
            affecting_divergences,
        }
    }

    /// Get divergence timeline for a specific link
    ///
    /// Returns all divergences for a link in chronological order, showing when
    /// prediction first diverged from reality.
    pub fn divergence_timeline(&self, link_id: &LinkId) -> Vec<&MetricDivergence> {
        let mut divergences: Vec<_> = self
            .snapshots
            .values()
            .flat_map(|s| s.divergence_log.iter())
            .filter(|d| d.link_id == *link_id)
            .collect();

        divergences.sort_by_key(|d| d.bitemporal_stamp.reception_time.as_datetime());
        divergences
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_snapshot_manager() {
        let mut manager = SnapshotManager::new(10);
        let base_time = Utc::now();

        // Store some snapshots
        for i in 0..5 {
            let snapshot = TwinSnapshot::new(
                base_time + Duration::seconds(i),
                World::new(),
                Vec::new(),
            );
            manager.store_snapshot(snapshot);
        }

        assert_eq!(manager.snapshot_count(), 5);

        // Test retrieval
        let snapshot = manager.get_snapshot(base_time + Duration::seconds(2));
        assert!(snapshot.is_some());

        // Test closest snapshot
        let closest = manager.get_closest_snapshot(base_time + Duration::milliseconds(2500));
        assert!(closest.is_some());
        assert_eq!(closest.unwrap().timestamp, base_time + Duration::seconds(2));
    }

    #[test]
    fn test_snapshot_limit() {
        let mut manager = SnapshotManager::new(3);
        let base_time = Utc::now();

        // Store more snapshots than the limit
        for i in 0..5 {
            let snapshot = TwinSnapshot::new(
                base_time + Duration::seconds(i),
                World::new(),
                Vec::new(),
            );
            manager.store_snapshot(snapshot);
        }

        // Should only retain the 3 most recent
        assert_eq!(manager.snapshot_count(), 3);
        assert!(manager.get_snapshot(base_time).is_none());
        assert!(manager.get_snapshot(base_time + Duration::seconds(1)).is_none());
        assert!(manager.get_snapshot(base_time + Duration::seconds(2)).is_some());
    }
}

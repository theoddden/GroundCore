//! Schedule data structures and management

use chrono::{DateTime, Utc};
use ground_core::{CustomerId, PassId, Result, SatelliteId};
use serde::{Deserialize, Serialize};

/// Pass request from a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassRequest {
    /// Unique request ID
    pub request_id: String,
    /// Customer ID
    pub customer_id: CustomerId,
    /// Satellite ID
    pub satellite_id: SatelliteId,
    /// Desired pass window
    pub window: PassWindow,
    /// Required hardware
    pub hardware_requirements: HardwareRequirements,
    /// Priority level
    pub priority: Priority,
    /// SLA tier (affects shadow allocation)
    pub sla_tier: SlaTier,
    /// When this request was submitted
    pub submitted_at: DateTime<Utc>,
}

/// Pass time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassWindow {
    /// Earliest acceptable start time
    pub start: DateTime<Utc>,
    /// Latest acceptable end time
    pub end: DateTime<Utc>,
    /// Minimum duration required (seconds)
    pub min_duration_sec: u64,
}

/// Hardware requirements for a pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareRequirements {
    /// Required frequency band
    pub frequency_band: String,
    /// Minimum sample rate
    pub min_sample_rate: u64,
    /// Whether shadow SDR is required
    pub require_shadow: bool,
    /// Required antenna type
    pub antenna_type: Option<String>,
}

/// Priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// SLA tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlaTier {
    BestEffort,
    Standard,
    Premium,
    MissionCritical,
}

impl SlaTier {
    /// Whether this tier requires shadow protection
    pub fn requires_shadow(&self) -> bool {
        matches!(self, SlaTier::Premium | SlaTier::MissionCritical)
    }
}

/// Scheduled pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledPass {
    /// Pass ID
    pub pass_id: PassId,
    /// Original request ID
    pub request_id: String,
    /// Customer ID
    pub customer_id: CustomerId,
    /// Satellite ID
    pub satellite_id: SatelliteId,
    /// Scheduled time window
    pub scheduled_window: PassWindow,
    /// Hardware allocation
    pub hardware_allocation: HardwareAllocation,
    /// When this was scheduled
    pub scheduled_at: DateTime<Utc>,
    /// Pass status
    pub status: PassStatus,
}

/// Hardware allocation for a pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAllocation {
    /// Allocated SDR device IDs
    pub sdr_devices: Vec<String>,
    /// Shadow SDR device ID (if any)
    pub shadow_sdr: Option<String>,
    /// Allocated antenna ID
    pub antenna_id: String,
    /// Allocated rotator ID
    pub rotator_id: String,
}

/// Pass status in schedule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PassStatus {
    Scheduled,
    Preparing,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Complete schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Scheduled passes
    pub passes: Vec<ScheduledPass>,
    /// When this schedule was generated
    pub generated_at: DateTime<Utc>,
    /// Schedule horizon (start, end)
    pub horizon: (DateTime<Utc>, DateTime<Utc>),
    /// Schedule version
    pub version: u64,
}

impl Schedule {
    pub fn new(horizon_start: DateTime<Utc>, horizon_end: DateTime<Utc>) -> Self {
        Self {
            passes: Vec::new(),
            generated_at: Utc::now(),
            horizon: (horizon_start, horizon_end),
            version: 0,
        }
    }

    /// Add a pass to the schedule
    pub fn add_pass(&mut self, pass: ScheduledPass) {
        self.passes.push(pass);
    }

    /// Get passes for a specific customer
    pub fn get_customer_passes(&self, customer_id: &CustomerId) -> Vec<&ScheduledPass> {
        self.passes
            .iter()
            .filter(|p| &p.customer_id == customer_id)
            .collect()
    }

    /// Get passes for a specific satellite
    pub fn get_satellite_passes(&self, satellite_id: &SatelliteId) -> Vec<&ScheduledPass> {
        self.passes
            .iter()
            .filter(|p| &p.satellite_id == satellite_id)
            .collect()
    }

    /// Get passes in a time range
    pub fn get_passes_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&ScheduledPass> {
        self.passes
            .iter()
            .filter(|p| p.scheduled_window.start >= start && p.scheduled_window.end <= end)
            .collect()
    }

    /// Check for conflicts in the schedule.
    ///
    /// Replaces the O(n²) pairwise scan with a hardware-indexed approach:
    /// passes are grouped by each hardware device they use, then conflicts
    /// are checked only within each device group. This is O(n) for grouping
    /// and O(k²) within each device bucket where k << n in practice.
    pub fn check_conflicts(&self) -> Vec<ScheduleConflict> {
        use std::collections::HashMap;

        // Build index: device_id -> list of passes using that device
        let mut device_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, pass) in self.passes.iter().enumerate() {
            for device in &pass.hardware_allocation.sdr_devices {
                device_index.entry(device.clone()).or_default().push(idx);
            }
            if let Some(shadow) = &pass.hardware_allocation.shadow_sdr {
                device_index.entry(shadow.clone()).or_default().push(idx);
            }
            device_index
                .entry(pass.hardware_allocation.antenna_id.clone())
                .or_default()
                .push(idx);
            device_index
                .entry(pass.hardware_allocation.rotator_id.clone())
                .or_default()
                .push(idx);
        }

        let mut conflicts = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // For each device, check all pairs of passes sharing that device
        for pass_indices in device_index.values() {
            for i in 0..pass_indices.len() {
                for j in (i + 1)..pass_indices.len() {
                    let (a, b) = (pass_indices[i], pass_indices[j]);
                    let key = if a < b { (a, b) } else { (b, a) };
                    if seen.insert(key) {
                        let pass1 = &self.passes[a];
                        let pass2 = &self.passes[b];
                        if windows_overlap(&pass1.scheduled_window, &pass2.scheduled_window) {
                            conflicts.push(ScheduleConflict {
                                pass1_id: pass1.pass_id.clone(),
                                pass2_id: pass2.pass_id.clone(),
                                conflict_type: ConflictType::HardwareOverlap,
                            });
                        }
                    }
                }
            }
        }

        conflicts
    }

    /// Get schedule statistics
    pub fn stats(&self) -> ScheduleStats {
        let total_passes = self.passes.len();
        let customers: std::collections::HashSet<_> =
            self.passes.iter().map(|p| &p.customer_id).collect();
        let satellites: std::collections::HashSet<_> =
            self.passes.iter().map(|p| &p.satellite_id).collect();

        let status_counts =
            self.passes
                .iter()
                .fold(std::collections::HashMap::new(), |mut acc, pass| {
                    *acc.entry(pass.status).or_insert(0) += 1;
                    acc
                });

        ScheduleStats {
            total_passes,
            unique_customers: customers.len(),
            unique_satellites: satellites.len(),
            status_counts,
            conflicts: self.check_conflicts().len(),
        }
    }
}

/// Schedule conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConflict {
    pub pass1_id: PassId,
    pub pass2_id: PassId,
    pub conflict_type: ConflictType,
}

/// Type of conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    HardwareOverlap,
    TimeOverlap,
    FrequencyConflict,
}

/// Check if two time windows overlap
fn windows_overlap(w1: &PassWindow, w2: &PassWindow) -> bool {
    w1.start < w2.end && w2.start < w1.end
}

/// Check if hardware allocations conflict
fn hardware_conflicts(a1: &HardwareAllocation, a2: &HardwareAllocation) -> bool {
    // Check SDR overlap
    for sdr1 in &a1.sdr_devices {
        if a2.sdr_devices.contains(sdr1) {
            return true;
        }
    }

    // Check shadow SDR overlap
    if let (Some(s1), Some(s2)) = (&a1.shadow_sdr, &a2.shadow_sdr) {
        if s1 == s2 {
            return true;
        }
    }

    // Check antenna overlap
    if a1.antenna_id == a2.antenna_id {
        return true;
    }

    // Check rotator overlap
    if a1.rotator_id == a2.rotator_id {
        return true;
    }

    false
}

/// Schedule statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleStats {
    pub total_passes: usize,
    pub unique_customers: usize,
    pub unique_satellites: usize,
    pub status_counts: std::collections::HashMap<PassStatus, usize>,
    pub conflicts: usize,
}

// Acquisition scheduling
//
// The scheduler manages the queue of acquisition requests and determines when
// each acquisition should begin based on timing constraints, resource availability,
// and priority.

use crate::pat::coordinator::PatError;
use crate::pat::{AcquisitionId, AcquisitionPlan};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Acquisition queue entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueEntry {
    plan: AcquisitionPlan,
    priority: u32,
    queued_at: DateTime<Utc>,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.plan.plan_id == other.plan.plan_id
    }
}

impl Eq for QueueEntry {}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority (lower number) comes first
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => {
                // If equal priority, earlier queued time comes first
                self.queued_at.cmp(&other.queued_at)
            }
            other => other,
        }
    }
}

/// Acquisition scheduler
pub struct AcquisitionScheduler {
    queue: BinaryHeap<QueueEntry>,
    scheduled: Vec<AcquisitionPlan>,
    in_progress: Option<AcquisitionId>,
}

impl AcquisitionScheduler {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            scheduled: Vec::new(),
            in_progress: None,
        }
    }

    /// Queue an acquisition plan
    pub fn queue(&mut self, plan: AcquisitionPlan, priority: u32) -> Result<(), PatError> {
        let entry = QueueEntry {
            plan,
            priority,
            queued_at: Utc::now(),
        };
        self.queue.push(entry);
        Ok(())
    }

    /// Schedule the next acquisition from the queue
    pub fn schedule_next(&mut self, _now: DateTime<Utc>) -> Option<AcquisitionPlan> {
        if let Some(entry) = self.queue.pop() {
            self.in_progress = Some(entry.plan.plan_id);
            self.scheduled.push(entry.plan.clone());
            Some(entry.plan)
        } else {
            None
        }
    }

    /// Mark an acquisition as complete
    pub fn complete(&mut self, acquisition_id: AcquisitionId) {
        if self.in_progress == Some(acquisition_id) {
            self.in_progress = None;
        }
    }

    /// Cancel a queued acquisition
    pub fn cancel(&mut self, acquisition_id: AcquisitionId) -> Result<(), PatError> {
        // Remove from queue if present
        let mut temp_queue = BinaryHeap::new();
        let mut found = false;

        while let Some(entry) = self.queue.pop() {
            if entry.plan.plan_id == acquisition_id {
                found = true;
            } else {
                temp_queue.push(entry);
            }
        }

        self.queue = temp_queue;

        if found {
            Ok(())
        } else {
            Err(PatError::NotFound(acquisition_id))
        }
    }

    /// Get the number of queued acquisitions
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Get the number of scheduled acquisitions
    pub fn scheduled_len(&self) -> usize {
        self.scheduled.len()
    }

    /// Check if an acquisition is currently in progress
    pub fn is_in_progress(&self) -> bool {
        self.in_progress.is_some()
    }

    /// Get the current in-progress acquisition ID
    pub fn in_progress_id(&self) -> Option<AcquisitionId> {
        self.in_progress
    }
}

impl Default for AcquisitionScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Acquisition queue (simplified interface)
pub struct AcquisitionQueue {
    scheduler: AcquisitionScheduler,
}

impl AcquisitionQueue {
    pub fn new() -> Self {
        Self {
            scheduler: AcquisitionScheduler::new(),
        }
    }

    pub fn add(&mut self, plan: AcquisitionPlan, priority: u32) -> Result<(), PatError> {
        self.scheduler.queue(plan, priority)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<AcquisitionPlan> {
        self.scheduler.schedule_next(Utc::now())
    }

    pub fn complete(&mut self, acquisition_id: AcquisitionId) {
        self.scheduler.complete(acquisition_id)
    }

    pub fn cancel(&mut self, acquisition_id: AcquisitionId) -> Result<(), PatError> {
        self.scheduler.cancel(acquisition_id)
    }

    pub fn len(&self) -> usize {
        self.scheduler.queue_len()
    }

    pub fn is_empty(&self) -> bool {
        self.scheduler.queue_len() == 0
    }
}

impl Default for AcquisitionQueue {
    fn default() -> Self {
        Self::new()
    }
}

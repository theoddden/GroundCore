// Safe mode and recovery procedures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Safe mode state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafeModeState {
    Normal,
    EnteringSafeMode,
    InSafeMode {
        reason: String,
        since: DateTime<Utc>,
    },
    Recovering,
    Failed,
}

/// Recovery procedure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryProcedure {
    pub procedure_id: String,
    pub steps: Vec<RecoveryStep>,
    pub current_step: usize,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: bool,
}

impl RecoveryProcedure {
    pub fn new(procedure_id: String, steps: Vec<RecoveryStep>) -> Self {
        Self {
            procedure_id,
            steps,
            current_step: 0,
            started_at: None,
            completed_at: None,
            success: false,
        }
    }

    pub fn start(&mut self) {
        self.started_at = Some(Utc::now());
        self.current_step = 0;
    }

    pub fn next_step(&mut self) -> Option<&RecoveryStep> {
        if self.current_step < self.steps.len() {
            let step = &self.steps[self.current_step];
            self.current_step += 1;
            Some(step)
        } else {
            None
        }
    }

    pub fn complete(&mut self, success: bool) {
        self.completed_at = Some(Utc::now());
        self.success = success;
    }

    pub fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    pub fn progress(&self) -> f64 {
        if self.steps.is_empty() {
            1.0
        } else {
            self.current_step as f64 / self.steps.len() as f64
        }
    }
}

/// Recovery step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStep {
    pub step_id: String,
    pub description: String,
    pub action: RecoveryAction,
    pub timeout_ms: u64,
}

/// Recovery action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    PowerCycle,
    ResetFirmware,
    Recalibrate,
    RestoreDefaults,
    Diagnostics,
}

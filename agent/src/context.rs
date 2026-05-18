//! Agent context for natural language queries
//!
//! The agent can answer operator questions by querying the system state
//! and synthesizing responses.

use crate::observation::{ObservationTool, SystemState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent context
pub struct AgentContext {
    observation_tool: Box<dyn ObservationTool>,
    context_window: Vec<ContextEntry>,
}

/// Context entry for conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    pub timestamp: DateTime<Utc>,
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    User,
    Agent,
}

/// Query result from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Answer to the query
    pub answer: String,
    /// Supporting evidence
    pub evidence: Vec<String>,
    /// Related system state
    pub system_state: Option<SystemState>,
    /// Confidence in the answer
    pub confidence: f64,
}

impl AgentContext {
    pub fn new(observation_tool: Box<dyn ObservationTool>) -> Self {
        Self {
            observation_tool,
            context_window: Vec::new(),
        }
    }

    /// Add a context entry
    pub fn add_context(&mut self, role: Role, content: String) {
        self.context_window.push(ContextEntry {
            timestamp: Utc::now(),
            role,
            content,
        });

        // Keep context window bounded
        if self.context_window.len() > 100 {
            self.context_window.remove(0);
        }
    }

    /// Process a user query
    pub async fn query(&mut self, question: String) -> QueryResult {
        self.add_context(Role::User, question.clone());

        // Get current system state
        let system_state = self.observation_tool.get_system_state();

        // In a real implementation, this would use Claude Code to reason
        // over the context and system state to generate an answer
        let answer = self.generate_answer(&question, system_state.as_ref().ok());

        self.add_context(Role::Agent, answer.clone());

        QueryResult {
            answer,
            evidence: Vec::new(),
            system_state: system_state.ok(),
            confidence: 0.8,
        }
    }

    /// Generate an answer (simplified implementation)
    fn generate_answer(&self, question: &str, state: Option<&SystemState>) -> String {
        if let Some(state) = state {
            if question.contains("active") && question.contains("pass") {
                format!(
                    "Currently {} active passes. Next pass at {:?}.",
                    state.active_passes.len(),
                    state.schedule_state.next_pass_time
                )
            } else if question.contains("hardware") {
                format!(
                    "Hardware state: {} SDRs, {} antennas, {} rotators. {} emergency shards available.",
                    state.hardware_state.sdr_devices.len(),
                    state.hardware_state.antennas.len(),
                    state.hardware_state.rotators.len(),
                    state.hardware_state.emergency_shards_available
                )
            } else if question.contains("telemetry") {
                format!(
                    "Average SNR: {:.1} dB, CPU usage: {:.0}%, Memory usage: {:.0}%",
                    state.telemetry.signal_metrics.average_snr,
                    state.telemetry.system_metrics.cpu_usage * 100.0,
                    state.telemetry.system_metrics.memory_usage * 100.0
                )
            } else {
                "I can answer questions about active passes, hardware state, telemetry, and scheduling. What would you like to know?".to_string()
            }
        } else {
            "Unable to retrieve system state. Please check system health.".to_string()
        }
    }

    /// Explain a decision or event
    pub fn explain(&self, event_id: String) -> QueryResult {
        // In a real implementation, this would load historical context
        // and use the agent to explain what happened and why
        QueryResult {
            answer: format!(
                "Event {}: This would be explained using historical context and bi-temporal logs.",
                event_id
            ),
            evidence: vec!["Bi-temporal log entry".to_string()],
            system_state: None,
            confidence: 0.7,
        }
    }

    /// Draft an incident report
    pub fn draft_incident_report(&self, incident_data: serde_json::Value) -> String {
        // In a real implementation, this would use the agent to synthesize
        // a coherent incident report from the provided data
        format!(
            "Incident Report\n===============\nTimestamp: {}\nSummary: [Agent would synthesize this]\nDetails: [Agent would synthesize this]\nRecommendations: [Agent would synthesize this]",
            Utc::now()
        )
    }
}

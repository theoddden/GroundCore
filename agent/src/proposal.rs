//! Action proposals with operator approval
//!
//! For consequential actions, the agent proposes and waits for operator approval.
//! For routine actions, the agent can act autonomously (with logging).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Action proposal from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionProposal {
    /// Proposal ID
    pub proposal_id: String,
    /// When this proposal was created
    pub created_at: DateTime<Utc>,
    /// Proposal type
    pub proposal_type: ProposalType,
    /// Description of the proposed action
    pub description: String,
    /// Rationale for the proposal
    pub rationale: String,
    /// Expected impact
    pub expected_impact: ImpactAssessment,
    /// Approval status
    pub approval_status: ApprovalStatus,
    /// When approved/rejected
    pub decided_at: Option<DateTime<Utc>>,
    /// Who approved/rejected
    pub decided_by: Option<String>,
    /// Execution result (if executed)
    pub execution_result: Option<ExecutionResult>,
}

/// Type of proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    /// Hardware reallocation
    HardwareReallocation {
        from_pass: String,
        to_pass: String,
        device_id: String,
    },
    /// Schedule adjustment
    ScheduleAdjustment {
        pass_id: String,
        new_time: DateTime<Utc>,
    },
    /// Failover initiation
    FailoverInitiation {
        pass_id: String,
        reason: String,
    },
    /// System configuration change
    ConfigChange {
        component: String,
        parameter: String,
        new_value: serde_json::Value,
    },
    /// Alert generation
    AlertGeneration {
        alert: String,
        severity: String,
    },
}

/// Impact assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    /// Affected passes
    pub affected_passes: Vec<String>,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Potential benefits
    pub benefits: Vec<String>,
    /// Potential drawbacks
    pub drawbacks: Vec<String>,
}

/// Risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Approval status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Output from execution
    pub output: String,
    /// When execution completed
    pub completed_at: DateTime<Utc>,
}

/// Proposal manager
pub struct ProposalManager {
    proposals: std::collections::HashMap<String, ActionProposal>,
}

impl ProposalManager {
    pub fn new() -> Self {
        Self {
            proposals: std::collections::HashMap::new(),
        }
    }
    
    /// Create a new proposal
    pub fn create_proposal(&mut self, proposal: ActionProposal) -> String {
        let id = proposal.proposal_id.clone();
        self.proposals.insert(id.clone(), proposal);
        id
    }
    
    /// Get a proposal
    pub fn get_proposal(&self, proposal_id: &str) -> Option<&ActionProposal> {
        self.proposals.get(proposal_id)
    }
    
    /// Approve a proposal
    pub fn approve(&mut self, proposal_id: &str, approved_by: String) -> Result<(), String> {
        if let Some(proposal) = self.proposals.get_mut(proposal_id) {
            proposal.approval_status = ApprovalStatus::Approved;
            proposal.decided_at = Some(Utc::now());
            proposal.decided_by = Some(approved_by);
            Ok(())
        } else {
            Err("Proposal not found".to_string())
        }
    }
    
    /// Reject a proposal
    pub fn reject(&mut self, proposal_id: &str, rejected_by: String) -> Result<(), String> {
        if let Some(proposal) = self.proposals.get_mut(proposal_id) {
            proposal.approval_status = ApprovalStatus::Rejected;
            proposal.decided_at = Some(Utc::now());
            proposal.decided_by = Some(rejected_by);
            Ok(())
        } else {
            Err("Proposal not found".to_string())
        }
    }
    
    /// Execute an approved proposal
    pub fn execute(&mut self, proposal_id: &str) -> Result<(), String> {
        if let Some(proposal) = self.proposals.get_mut(proposal_id) {
            if proposal.approval_status != ApprovalStatus::Approved {
                return Err("Proposal not approved".to_string());
            }
            
            // In a real implementation, this would execute the action
            proposal.approval_status = ApprovalStatus::Executed;
            proposal.execution_result = Some(ExecutionResult {
                success: true,
                output: "Action executed successfully".to_string(),
                completed_at: Utc::now(),
            });
            
            Ok(())
        } else {
            Err("Proposal not found".to_string())
        }
    }
    
    /// Get pending proposals
    pub fn pending_proposals(&self) -> Vec<&ActionProposal> {
        self.proposals
            .values()
            .filter(|p| p.approval_status == ApprovalStatus::Pending)
            .collect()
    }
    
    /// Get proposals requiring approval (consequential actions)
    pub fn requires_approval(&self) -> Vec<&ActionProposal> {
        self.proposals
            .values()
            .filter(|p| {
                p.approval_status == ApprovalStatus::Pending
                    && matches!(p.expected_impact.risk_level, RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical)
            })
            .collect()
    }
    
    /// Get autonomous actions (low risk, can execute without approval)
    pub fn autonomous_actions(&self) -> Vec<&ActionProposal> {
        self.proposals
            .values()
            .filter(|p| {
                p.approval_status == ApprovalStatus::Pending
                    && matches!(p.expected_impact.risk_level, RiskLevel::Low)
            })
            .collect()
    }
}

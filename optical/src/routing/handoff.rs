// Handoff strategy and triggers
//
// Pre-emptive handoffs are critical for optical links because the acquisition
// time is long (seconds to minutes). The system must initiate handoffs before
// the current link fails.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Handoff strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandoffStrategy {
    /// Seamless handoff with zero packet loss
    Seamless { overlap_ms: u64 },
    
    /// Make-before-break handoff
    MakeBeforeBreak { overlap_ms: u64 },
    
    /// Break-before-make handoff
    BreakBeforeMake { gap_ms: u64 },
    
    /// No handoff - let link fail naturally
    NoHandoff,
}

impl HandoffStrategy {
    pub fn seamless(overlap_ms: u64) -> Self {
        Self::Seamless { overlap_ms }
    }

    pub fn make_before_break(overlap_ms: u64) -> Self {
        Self::MakeBeforeBreak { overlap_ms }
    }

    pub fn break_before_make(gap_ms: u64) -> Self {
        Self::BreakBeforeMake { gap_ms }
    }

    pub fn no_handoff() -> Self {
        Self::NoHandoff
    }

    pub fn requires_overlap(&self) -> bool {
        matches!(self, Self::Seamless { .. } | Self::MakeBeforeBreak { .. })
    }
}

/// Handoff trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandoffTrigger {
    /// Trigger based on link quality degradation
    QualityThreshold { ber: f64, snr_db: f64 },
    
    /// Trigger based on time to link loss
    TimeToLoss { remaining_ms: u64 },
    
    /// Trigger based on scheduled maintenance
    Scheduled { at: DateTime<Utc> },
    
    /// Manual trigger
    Manual { reason: String },
    
    /// Trigger based on better alternative link
    BetterLinkAvailable { improvement_factor: f64 },
}

impl HandoffTrigger {
    pub fn quality_threshold(ber: f64, snr_db: f64) -> Self {
        Self::QualityThreshold { ber, snr_db }
    }

    pub fn time_to_loss(remaining_ms: u64) -> Self {
        Self::TimeToLoss { remaining_ms }
    }

    pub fn scheduled(at: DateTime<Utc>) -> Self {
        Self::Scheduled { at }
    }

    pub fn manual(reason: String) -> Self {
        Self::Manual { reason }
    }

    pub fn better_link_available(improvement_factor: f64) -> Self {
        Self::BetterLinkAvailable { improvement_factor }
    }
}

/// Handoff decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffDecision {
    pub trigger: HandoffTrigger,
    pub strategy: HandoffStrategy,
    pub target_link: Option<String>,
    pub initiated_at: DateTime<Utc>,
    pub reason: String,
}

impl HandoffDecision {
    pub fn new(trigger: HandoffTrigger, strategy: HandoffStrategy, reason: String) -> Self {
        Self {
            trigger,
            strategy,
            target_link: None,
            initiated_at: Utc::now(),
            reason,
        }
    }

    pub fn with_target(mut self, target_link: String) -> Self {
        self.target_link = Some(target_link);
        self
    }
}

// Link State - bi-temporal link phase tracking
//
// Link state with bi-temporal observation matters for post-incident analysis:
// when a link fails at 14:32:18 UTC, the answer requires knowing both when the
// system observed the failure and when the failure actually occurred.

use crate::{LinkId, TerminalId, DataRate, BiTemporal, TrackingQuality, DegradationReason, LossCause};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

/// Link phase with bi-temporal timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "phase")]
pub enum LinkPhase {
    Idle,
    PatScheduled { acquisition_start: DateTime<Utc> },
    Acquiring {
        since: BiTemporal<DateTime<Utc>>,
        peer_attestation: Option<PeerHandshake>,
    },
    Tracking {
        quality: TrackingQuality,
        established_at: BiTemporal<DateTime<Utc>>,
    },
    Communicating {
        fec_state: FecState,
        throughput: DataRate,
    },
    Degrading {
        reason: DegradationReason,
        predicted_failure: Option<DateTime<Utc>>,
    },
    Lost {
        cause: LossCause,
        last_known: BiTemporal<DateTime<Utc>>,
    },
}

/// Peer handshake during acquisition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerHandshake {
    pub peer_terminal: TerminalId,
    pub handshake_timestamp: DateTime<Utc>,
    pub handshake_signature: Vec<u8>,
}

/// FEC state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FecState {
    Disabled,
    Enabled,
    Degraded,
}

/// Link metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkMetrics {
    pub bit_error_rate: f64,
    pub signal_to_noise_db: f64,
    pub latency_ms: u64,
    pub throughput_bps: u64,
    pub power_watts: f64,
    pub temperature_c: f64,
    pub last_updated: DateTime<Utc>,
}

/// Phase transition with bi-temporal recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseTransition {
    pub from_phase: LinkPhase,
    pub to_phase: LinkPhase,
    pub transition_time: BiTemporal<DateTime<Utc>>,
    pub trigger: PhaseTransitionTrigger,
}

/// What caused the phase transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseTransitionTrigger {
    Scheduled,
    AcquisitionSuccess,
    AcquisitionTimeout,
    SignalDegraded,
    SignalLost,
    ManualCommand,
    PowerCycle,
    Error(String),
}

/// Active link with full state history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLink {
    pub link_id: LinkId,
    pub endpoints: (TerminalId, TerminalId),
    pub phase: LinkPhase,
    pub phase_history: Vec<PhaseTransition>,
    pub metrics: LinkMetrics,
}

impl ActiveLink {
    pub fn new(link_id: LinkId, endpoints: (TerminalId, TerminalId)) -> Self {
        Self {
            link_id,
            endpoints,
            phase: LinkPhase::Idle,
            phase_history: vec![],
            metrics: LinkMetrics {
                bit_error_rate: 0.0,
                signal_to_noise_db: 0.0,
                latency_ms: 0,
                throughput_bps: 0,
                power_watts: 0.0,
                temperature_c: 0.0,
                last_updated: Utc::now(),
            },
        }
    }

    /// Transition to new phase with bi-temporal recording
    pub fn transition_to(&mut self, new_phase: LinkPhase, trigger: PhaseTransitionTrigger) {
        let now = Utc::now();
        let transition = PhaseTransition {
            from_phase: std::mem::replace(&mut self.phase, new_phase.clone()),
            to_phase: new_phase,
            transition_time: BiTemporal::new(now, now),
            trigger,
        };
        self.phase_history.push(transition);
    }

    /// Update metrics
    pub fn update_metrics(&mut self, new_metrics: LinkMetrics) {
        self.metrics = new_metrics;
    }

    /// Get current phase
    pub fn current_phase(&self) -> &LinkPhase {
        &self.phase
    }

    /// Check if link is active (tracking or communicating)
    pub fn is_active(&self) -> bool {
        matches!(self.phase, LinkPhase::Tracking { .. } | LinkPhase::Communicating { .. })
    }

    /// Check if link is in failure state
    pub fn is_failed(&self) -> bool {
        matches!(self.phase, LinkPhase::Lost { .. })
    }
}

impl Default for LinkMetrics {
    fn default() -> Self {
        Self {
            bit_error_rate: 0.0,
            signal_to_noise_db: 0.0,
            latency_ms: 0,
            throughput_bps: 0,
            power_watts: 0.0,
            temperature_c: 0.0,
            last_updated: Utc::now(),
        }
    }
}

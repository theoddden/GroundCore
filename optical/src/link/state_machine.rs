// Link state machine
//
// Extended with control plane traffic classification (KubeSpace-inspired):
// Optical links can carry control plane traffic with priority routing,
// enabling lower latency than RF for satellite management.

use crate::BiTemporal;
use crate::oct::OctConfiguration;
use crate::pat::AcquisitionPlan;
use bitemporal::{EventTime, ReceptionTime};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use snapshotting::manager::SnapshotManager;
use uuid::Uuid;

pub type LinkId = Uuid;

/// Link phase
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum LinkPhase {
    Scheduled {
        acquisition_plan: Box<AcquisitionPlan>,
    },
    Acquiring {
        since: DateTime<Utc>,
        search_state: String,
    },
    Established {
        since: DateTime<Utc>,
        quality: LinkQuality,
    },
    Degrading {
        reason: DegradationReason,
        action: DegradationAction,
    },
    Recovering {
        strategy: RecoveryStrategy,
    },
    Terminating {
        reason: TerminationReason,
    },
    Failed {
        cause: FailureCause,
    },
}

/// Link quality
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinkQuality {
    pub bit_error_rate: f64,
    pub signal_to_noise_db: f64,
    pub pointing_error_urad: f64,
}

impl LinkQuality {
    pub fn excellent() -> Self {
        Self {
            bit_error_rate: 1e-12,
            signal_to_noise_db: 20.0,
            pointing_error_urad: 10.0,
        }
    }

    pub fn good() -> Self {
        Self {
            bit_error_rate: 1e-9,
            signal_to_noise_db: 15.0,
            pointing_error_urad: 50.0,
        }
    }

    pub fn degraded() -> Self {
        Self {
            bit_error_rate: 1e-6,
            signal_to_noise_db: 8.0,
            pointing_error_urad: 150.0,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.bit_error_rate < 1e-8 && self.signal_to_noise_db > 10.0
    }
}

/// Degradation reason
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DegradationReason {
    AtmosphericTurbulence { severity: String },
    PointingDrift { drift_rate_urad_per_sec: f64 },
    AttenuationIncrease { delta_db: f64 },
    FecOverwhelmed { ber_trend: String },
    UnexplainedSnrDrop { magnitude_db: f64 },
}

/// Degradation action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DegradationAction {
    Monitor,
    InitiateHandoff,
    IncreasePower,
    AdjustModulation,
}

/// Recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    RetryAcquisition,
    SwitchTerminal,
    Reconfigure,
}

/// Termination reason
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationReason {
    Normal,
    Scheduled,
    Emergency,
}

/// Failure cause
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureCause {
    AcquisitionTimeout,
    HardwareFailure,
    PowerLoss,
    SignalLost,
}

/// Optical link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpticalLink {
    pub link_id: LinkId,
    pub endpoints: (String, String),
    pub phase: LinkPhase,
    pub established_at: Option<BiTemporal<DateTime<Utc>>>,
    pub current_metrics: LinkMetrics,
    pub negotiated_config: OctConfiguration,
    /// Snapshot manager for link state failover
    #[serde(skip)]
    snapshot_manager: Option<SnapshotManager>,
}

impl OpticalLink {
    pub fn new(link_id: LinkId, endpoints: (String, String), config: OctConfiguration) -> Self {
        Self {
            link_id,
            endpoints,
            phase: LinkPhase::Failed {
                cause: FailureCause::AcquisitionTimeout,
            },
            established_at: None,
            current_metrics: LinkMetrics::default(),
            negotiated_config: config,
            snapshot_manager: Some(SnapshotManager::new(10)),
        }
    }

    pub fn schedule(&mut self, acquisition_plan: AcquisitionPlan) {
        self.phase = LinkPhase::Scheduled { acquisition_plan };
    }

    pub fn transition_to_acquiring(&mut self) {
        self.phase = LinkPhase::Acquiring {
            since: Utc::now(),
            search_state: "initial".to_string(),
        };
    }

    pub fn transition_to_established(&mut self, quality: LinkQuality) {
        let now = Utc::now();
        self.established_at = Some(BiTemporal::new(
            now,
            EventTime::new(now),
            ReceptionTime::new(now),
        ));
        self.phase = LinkPhase::Established {
            since: now,
            quality,
        };
    }

    pub fn transition_to_degrading(
        &mut self,
        reason: DegradationReason,
        action: DegradationAction,
    ) {
        self.phase = LinkPhase::Degrading { reason, action };
    }

    pub fn transition_to_failed(&mut self, cause: FailureCause) {
        self.phase = LinkPhase::Failed { cause };
    }

    pub fn is_established(&self) -> bool {
        matches!(self.phase, LinkPhase::Established { .. })
    }

    pub fn is_healthy(&self) -> bool {
        if let LinkPhase::Established { quality, .. } = &self.phase {
            quality.is_healthy()
        } else {
            false
        }
    }

    /// Snapshot current link state for failover recovery
    pub fn snapshot(&mut self) -> Result<(), String> {
        if let Some(mgr) = self.snapshot_manager.as_mut() {
            // Use LinkPhase as a proxy value — serialize the phase for the snapshot
            let phase_label = format!("{:?}", self.phase);
            mgr.schedule().snapshot(&phase_label);
            tracing::info!(
                "Snapshot link {}: phase={:?}, quality={:.1}dB",
                self.link_id,
                self.phase,
                self.current_metrics.signal_quality_db,
            );
        }
        Ok(())
    }

    /// Get snapshot manager for direct access
    pub fn snapshot_manager(&mut self) -> Option<&mut SnapshotManager> {
        self.snapshot_manager.as_mut()
    }
}

/// Link metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkMetrics {
    pub data_rate_actual: u64,
    pub data_rate_capacity: u64,
    pub bit_error_rate: f64,
    pub fec_corrections_per_second: u64,
    pub arq_retransmits_per_second: u64,
    pub signal_quality_db: f64,
    pub pointing_error_urad: f64,
    /// Control plane traffic metrics
    pub control_plane_traffic: ControlPlaneTraffic,
}

/// Control plane traffic metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneTraffic {
    pub enabled: bool,
    pub priority: TrafficPriority,
    pub data_rate_actual: u64,
    pub latency_ms: f64,
    pub packets_per_second: u64,
}

/// Traffic priority for control plane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrafficPriority {
    Critical,
    High,
    Normal,
    Low,
}

impl ControlPlaneTraffic {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            priority: TrafficPriority::Normal,
            data_rate_actual: 0,
            latency_ms: 0.0,
            packets_per_second: 0,
        }
    }

    pub fn enabled(priority: TrafficPriority) -> Self {
        Self {
            enabled: true,
            priority,
            data_rate_actual: 0,
            latency_ms: 0.0,
            packets_per_second: 0,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for LinkMetrics {
    fn default() -> Self {
        Self {
            data_rate_actual: 0,
            data_rate_capacity: 10_000_000_000,
            bit_error_rate: 0.0,
            fec_corrections_per_second: 0,
            arq_retransmits_per_second: 0,
            signal_quality_db: 0.0,
            pointing_error_urad: 0.0,
            control_plane_traffic: ControlPlaneTraffic::disabled(),
        }
    }
}

impl LinkMetrics {
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::default()
    }

    pub fn utilization(&self) -> f64 {
        if self.data_rate_capacity > 0 {
            self.data_rate_actual as f64 / self.data_rate_capacity as f64
        } else {
            0.0
        }
    }

    /// Enable control plane traffic on this link
    pub fn enable_control_plane(&mut self, priority: TrafficPriority) {
        self.control_plane_traffic = ControlPlaneTraffic::enabled(priority);
    }

    /// Disable control plane traffic on this link
    pub fn disable_control_plane(&mut self) {
        self.control_plane_traffic = ControlPlaneTraffic::disabled();
    }

    /// Update control plane traffic metrics
    pub fn update_control_plane_metrics(
        &mut self,
        data_rate: u64,
        latency_ms: f64,
        packets_per_sec: u64,
    ) {
        self.control_plane_traffic.data_rate_actual = data_rate;
        self.control_plane_traffic.latency_ms = latency_ms;
        self.control_plane_traffic.packets_per_second = packets_per_sec;
    }
}

/// Link state (simplified)
pub type LinkState = OpticalLink;

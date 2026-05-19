// OISL - Optical Inter-Satellite Link Control Plane
//
// Five-plane architecture for constellation-scale optical communications:
// - Mission Plane: Intent-based mission planning and compilation
// - Topology Plane: Spatiotemporal graph forecasting and routing
// - Resource Plane: Per-satellite distributed resource management
// - Physical Plane: Vendor abstraction over SDA OCT standard
// - PAT Coordination: Hard real-time synchronized acquisition
// - Federation Plane: Cross-operator OISL coordination

pub mod federation;
pub mod mission;
pub mod pat;
pub mod physical;
pub mod resource;
pub mod topology;

// Re-export common types
pub use mission::{
    CompilationError, IntentCompiler, IntentConstraints, MissionIntent, ObjectiveType,
    PlanExplanation, ServiceLevelAgreement, TaskingPlan, ValidationWarning,
};

pub use topology::{
    ActiveLink, CostModel, GraphSnapshot, LinkMetrics, LinkPhase, PotentialEdge, Route, RoutedHop,
    SpatiotemporalRouter, TopologyForecast, TopologyForecaster,
};

pub use resource::manager::{
    ComputeResources, PowerBudget, StorageResources, TerminalCapability, ThermalState,
};
pub use resource::{
    DegradationForecast, ResourceAllocation, ResourceClaim, SatelliteNode, SatelliteScheduler,
};

pub use physical::{
    CondorMk3, FecCode, FecConfiguration, LinkType, Modulation, OctConfiguration, OpticalTerminal,
    Scot80, TelemetryStream,
};

pub use physical::terminal::TerminalCapability as PhysicalCapability;

pub use pat::{
    ClockConfidence, PatCoordinator, PatEventType, PrecisionClock, PrecisionTimestamp,
    ScheduledAcquisition,
};

pub use federation::{
    AttestationEngine, CrossOperatorLink, FederationPeer, FederationPlane, FederationPolicy,
    RevenueAgreement,
};

// Common type aliases
pub type IntentId = uuid::Uuid;
pub type PlanId = uuid::Uuid;
pub type TaskId = uuid::Uuid;
pub type AllocationId = uuid::Uuid;
pub type LinkId = uuid::Uuid;
pub type TerminalId = uuid::Uuid;
pub type AcquisitionId = uuid::Uuid;
pub type SatelliteId = String;
pub type NodeId = String;
pub type OperatorId = String;
pub type TenantId = String;
pub type AssetId = String;

// Common newtypes for type safety
/// Data rate in bits per second
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct DataRate(pub u64);

/// Frequency in Hz
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Frequency(pub u64);

/// Bytes
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Bytes(pub u64);

/// Confidence score (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceScore(pub f64);

impl ConfidenceScore {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    pub fn is_low(&self) -> bool {
        self.0 < 0.5
    }
}

/// Time window with start and end
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimeWindow {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

impl TimeWindow {
    pub fn new(start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Self {
        Self { start, end }
    }

    pub fn duration(&self) -> chrono::Duration {
        self.end - self.start
    }

    pub fn contains(&self, timestamp: chrono::DateTime<chrono::Utc>) -> bool {
        timestamp >= self.start && timestamp <= self.end
    }

    pub fn overlaps(&self, other: &TimeWindow) -> bool {
        self.start < other.end && self.end > other.start
    }
}

/// Geo region for observation missions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeoRegion {
    pub name: String,
    pub polygon: Vec<(f64, f64)>, // (latitude, longitude)
}

/// Sensor type for observation missions
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SensorType {
    Optical,
    Sar,
    Infrared,
    Hyperspectral,
}

/// Priority levels
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Background = 4,
}

/// Bandwidth allocation
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BandwidthAllocation {
    pub data_rate: DataRate,
    pub valid_window: TimeWindow,
}

/// Tracking quality metrics
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrackingQuality {
    pub pointing_error_rad: f64,
    pub signal_to_noise_db: f64,
    pub lock_confidence: ConfidenceScore,
}

/// Degradation reasons
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DegradationReason {
    AtmosphericTurbulence,
    ThermalStress,
    PowerLimitation,
    MechanicalMisalignment,
    ClockDrift,
    VendorSpecific(String),
}

/// Loss causes
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LossCause {
    PatTimeout,
    SignalDegraded,
    PowerLoss,
    MechanicalFailure,
    ClockDesync,
    VendorSpecific(String),
}

/// Fec state
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FecState {
    Disabled,
    Enabled,
    Degraded,
}

/// Preemption reasons
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PreemptionReason {
    HigherPriorityTask,
    EmergencyResponse,
    RegulatoryCompliance,
    ResourceExhaustion,
}

/// Reset levels for terminals
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ResetLevel {
    Soft,
    Hard,
    Factory,
}

/// OCT standard version
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OctStandardVersion {
    V3_0,
    V3_1,
    V3_2,
    V4_0_0,
}

/// LDPC variant (5G NR)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LdpcVariant {
    BaseGraph1,
    BaseGraph2,
}

/// Code rate for LDPC
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CodeRate {
    R1_2,
    R2_3,
    R3_4,
    R5_6,
}

/// Baud rate
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BaudRate(pub u64);

/// Geometry score for link quality
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GeometryScore {
    pub pointing_angle_rad: f64,
    pub doppler_shift_hz: f64,
    pub range_km: f64,
}

impl GeometryScore {
    pub fn overall_quality(&self) -> f64 {
        // Lower pointing angle, lower doppler, shorter range = higher quality
        let angle_score = 1.0 - (self.pointing_angle_rad / std::f64::consts::PI).min(1.0);
        let doppler_score = 1.0 - (self.doppler_shift_hz / 1e6).min(1.0);
        let range_score = 1.0 - (self.range_km / 10000.0).min(1.0);
        (angle_score + doppler_score + range_score) / 3.0
    }
}

/// ARQ configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArqConfiguration {
    pub enabled: bool,
    pub max_retransmissions: u8,
    pub timeout_ms: u64,
}

/// Acquisition sequence
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AcquisitionSequence {
    pub steps: Vec<AcquisitionStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AcquisitionStep {
    pub duration_ms: u64,
    pub action: AcquisitionAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AcquisitionAction {
    CoarsePoint,
    FinePoint,
    BeaconTransmit,
    BeaconReceive,
    Lock,
}

/// Fallback actions for failed acquisition
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FallbackAction {
    RetryWithWiderBeam,
    RetryAtLaterTime(chrono::DateTime<chrono::Utc>),
    UseAlternativeTerminal(TerminalId),
    Abort,
}

/// Time reference for clock synchronization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimeReference {
    pub source: String,
    pub accuracy_ns: u64,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// Re-export PointingVector from optical to eliminate the duplicate definition.
/// Using optical's canonical type prevents silent divergence between modules.
pub use optical::PointingVector;

/// Re-export BiTemporal from the shared bitemporal crate.
/// The previous local definition (valid_from/transaction_time) differed from
/// optical's (event_time/system_observation_time) — both are now replaced by
/// the authoritative bitemporal::BiTemporal.
pub use bitemporal::BiTemporal;

/// Health metrics wrapper
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthMetrics(pub ConfidenceScore);

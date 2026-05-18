// Optical Communications Module
//
// Three properties drive the entire module design:
// 1. Pointing precision matters at sub-microradian level
// 2. Acquisition is a multi-stage choreographed dance
// 3. The optical channel is binary (acquired or not)
//
// Module structure:
// - oct: SDA OCT Standard implementation
// - pat: Pointing, Acquisition, Tracking
// - geometry: Pointing math and visibility
// - link: Link lifecycle and state
// - terminal: Vendor abstraction
// - routing: Optical-aware routing
// - attestation: Bi-temporal proof primitives

pub mod oct;
pub mod pat;
pub mod geometry;
pub mod link;
pub mod terminal;
pub mod routing;
pub mod attestation;

// Re-export core types
pub use oct::{
    OctStandardVersion, OctConfiguration, LinkType, Modulation,
    FecConfiguration, FecCode, ArqConfiguration, TrackingTone,
    negotiate_version, NegotiationError,
};

pub use pat::{
    AcquisitionPlan, PatPhase, PatCoordinator, SearchPattern,
    PrecisionClock, PrecisionTimestamp, ClockConfidence, TimeReference,
    AcquisitionId, AcquisitionResult, TrackingHandle, RecoveryStrategy,
};

pub use geometry::{
    PointingVector, VisibilityWindow, VisibilityConstraints,
    compute_pointing_vector, predict_visibility_window,
    compute_optical_doppler, compute_atmospheric_attenuation,
};

pub use link::state_machine::{
    LinkPhase, LinkQuality, DegradationReason, DegradationAction, RecoveryStrategy as LinkRecoveryStrategy,
    TerminationReason, FailureCause, OpticalLink, LinkMetrics,
};
pub use link::metrics::{MetricSnapshot, BoundedHistory};
pub use link::degradation::DegradationDetector;
pub use link::failover::{FailoverStrategy as HandoffStrategy, FailoverManager};

pub use terminal::capability::{TerminalCapability, Vendor};
pub use terminal::trait_def::{CalibrationReport, EthernetEndpoint};
pub use terminal::telemetry::{TelemetryStream, HealthReport, ResetLevel};

pub use routing::{
    OpticalTopology, PathFinder, OpticalRoute,
};

pub use attestation::{
    LinkAttestation, AttestationId, AttestableEvent, Hash, Signature,
    TerminalIdentity, PatAttestation,
};

// Common type aliases
pub type LinkId = uuid::Uuid;
pub type TerminalId = uuid::Uuid;
pub type SatelliteId = String;
pub type TenantId = String;
pub type DataRate = u64;
pub type BaudRate = u64;
pub type Hz = u64;

// Common newtypes
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Microradians(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct AttenuationDb(pub f64);

// Bi-temporal timestamp for audit trails — use the canonical definition
// from the shared bitemporal crate to avoid semantic drift.
pub use bitemporal::{BiTemporal, EventTime, ReceptionTime};

// Error types
pub use error::OpticalError;

mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum OpticalError {
        #[error("OCT Standard error: {0}")]
        OctError(String),

        #[error("PAT error: {0}")]
        PatError(String),

        #[error("Geometry error: {0}")]
        GeometryError(String),

        #[error("Link error: {0}")]
        LinkError(String),

        #[error("Terminal error: {0}")]
        TerminalError(String),

        #[error("Routing error: {0}")]
        RoutingError(String),

        #[error("Attestation error: {0}")]
        AttestationError(String),

        #[error("Clock error: {0}")]
        ClockError(String),

        #[error("IO error: {0}")]
        IoError(#[from] std::io::Error),

        #[error("Serialization error: {0}")]
        SerializationError(#[from] serde_json::Error),
    }
}

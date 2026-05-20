// Pointing, Acquisition, Tracking — the hard real-time core
//
// The architectural challenge: two satellites with synchronized clocks must
// independently arrive at the same acquisition state at the same time without
// communicating.

pub mod calibration;
pub mod clock;
pub mod coordinator;
pub mod recovery;
pub mod scheduler;
pub mod search;
pub mod tracking;

pub use calibration::{
    CalibrationObservation, CalibrationSource, CalibrationTracker, PointingCorrection,
    PointingModel,
};
pub use clock::{ClockConfidence, DriftRate, PrecisionClock, PrecisionTimestamp, TimeReference};
pub use coordinator::{AcquisitionPlan, AcquisitionResult, PatCoordinator, PatPhase};
pub use recovery::{FallbackAction, LossReason, RecoveryStrategy};
pub use scheduler::{AcquisitionQueue, AcquisitionScheduler};
pub use search::{LissajousPattern, RasterPattern, SearchPattern, SearchState, SpiralPattern};
pub use tracking::{TrackingHandle, TrackingMetrics, TrackingQuality};

use uuid::Uuid;

pub type AcquisitionId = Uuid;

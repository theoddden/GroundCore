// Pointing, Acquisition, Tracking — the hard real-time core
//
// The architectural challenge: two satellites with synchronized clocks must
// independently arrive at the same acquisition state at the same time without
// communicating.

pub mod scheduler;
pub mod coordinator;
pub mod clock;
pub mod search;
pub mod tracking;
pub mod recovery;

pub use scheduler::{AcquisitionScheduler, AcquisitionQueue};
pub use coordinator::{PatCoordinator, AcquisitionPlan, PatPhase, AcquisitionResult};
pub use clock::{PrecisionClock, PrecisionTimestamp, ClockConfidence, TimeReference, DriftRate};
pub use search::{SearchPattern, SearchState, SpiralPattern, RasterPattern, LissajousPattern};
pub use tracking::{TrackingHandle, TrackingMetrics, TrackingQuality};
pub use recovery::{RecoveryStrategy, FallbackAction, LossReason};

use uuid::Uuid;

pub type AcquisitionId = Uuid;

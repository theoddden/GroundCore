// Precision time management
//
// The clock subsystem is foundational because PAT depends on synchronized timing.
// GPS-disciplined oscillators provide ~50 nanosecond accuracy.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClockError {
    #[error("Clock sync failed: {0}")]
    SyncFailed(String),

    #[error("Clock drift exceeded threshold: {0} ns/sec")]
    DriftExceeded(i64),

    #[error("Clock not synchronized")]
    NotSynchronized,
}

/// Precision timestamp with nanosecond precision and confidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecisionTimestamp {
    pub seconds: i64,
    pub nanoseconds: u32,
    pub confidence_ns: u32,
}

impl PrecisionTimestamp {
    pub fn now() -> Self {
        let now = Utc::now();
        let seconds = now.timestamp();
        let nanoseconds = now.timestamp_subsec_nanos();
        Self {
            seconds,
            nanoseconds,
            confidence_ns: 100, // Default 100ns uncertainty
        }
    }

    pub fn from_datetime(dt: DateTime<Utc>, confidence_ns: u32) -> Self {
        Self {
            seconds: dt.timestamp(),
            nanoseconds: dt.timestamp_subsec_nanos(),
            confidence_ns,
        }
    }

    pub fn to_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.seconds, self.nanoseconds).unwrap_or(Utc::now())
    }

    pub fn add_duration(&self, duration: Duration) -> Self {
        let dt = self.to_datetime() + duration;
        Self::from_datetime(dt, self.confidence_ns)
    }

    pub fn duration_since(&self, other: &Self) -> Duration {
        self.to_datetime() - other.to_datetime()
    }
}

/// Clock confidence in nanoseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockConfidence {
    pub uncertainty_ns: u32,
}

impl ClockConfidence {
    pub fn new(uncertainty_ns: u32) -> Self {
        Self { uncertainty_ns }
    }

    pub fn gps_disciplined() -> Self {
        Self::new(50)
    }

    pub fn atomic_clock() -> Self {
        Self::new(10)
    }

    pub fn ptp() -> Self {
        Self::new(100)
    }

    pub fn is_high_precision(&self) -> bool {
        self.uncertainty_ns < 100
    }

    pub fn is_acceptable_for_pat(&self) -> bool {
        self.uncertainty_ns < 500 // PAT requires < 500ns
    }
}

/// Clock drift rate in nanoseconds per second
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftRate {
    pub ns_per_second: i64,
}

impl DriftRate {
    pub fn zero() -> Self {
        Self { ns_per_second: 0 }
    }

    pub fn from_ppb(ppb: i64) -> Self {
        Self { ns_per_second: ppb }
    }

    pub fn is_acceptable(&self) -> bool {
        self.ns_per_second.abs() < 100 // < 100 ns/sec drift is acceptable
    }
}

/// Time reference source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeReference {
    Gps { receiver: String },
    Ptp { master: String },
    NtpStratum1 { server: String },
    AtomicClock { source: String },
}

impl TimeReference {
    pub fn accuracy_ns(&self) -> u32 {
        match self {
            Self::Gps { .. } => 50,
            Self::Ptp { .. } => 100,
            Self::NtpStratum1 { .. } => 1000,
            Self::AtomicClock { .. } => 10,
        }
    }
}

/// Sync report after clock synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub synced_at: DateTime<Utc>,
    pub offset_ns: i64,
    pub confidence_after: ClockConfidence,
    pub successful: bool,
}

/// Precision clock trait
pub trait PrecisionClock: Send + Sync {
    /// Get current precision timestamp
    fn now(&self) -> PrecisionTimestamp;

    /// Get clock confidence
    fn confidence(&self) -> ClockConfidence;

    /// Sync to time reference
    fn sync_to(&mut self, reference: &TimeReference) -> Result<SyncReport, ClockError>;

    /// Check if clock is synchronized
    fn is_synchronized(&self) -> bool;

    /// Get drift rate
    fn drift_rate(&self) -> DriftRate;
}

/// Default precision clock implementation
pub struct DefaultPrecisionClock {
    confidence: ClockConfidence,
    last_sync: Option<DateTime<Utc>>,
    drift_rate: DriftRate,
    synchronized: bool,
}

impl DefaultPrecisionClock {
    pub fn new() -> Self {
        Self {
            confidence: ClockConfidence::gps_disciplined(),
            last_sync: None,
            drift_rate: DriftRate::zero(),
            synchronized: false,
        }
    }
}

impl Default for DefaultPrecisionClock {
    fn default() -> Self {
        Self::new()
    }
}

impl PrecisionClock for DefaultPrecisionClock {
    fn now(&self) -> PrecisionTimestamp {
        PrecisionTimestamp::now()
    }

    fn confidence(&self) -> ClockConfidence {
        self.confidence
    }

    fn sync_to(&mut self, reference: &TimeReference) -> Result<SyncReport, ClockError> {
        let now = Utc::now();
        let offset_ns = reference.accuracy_ns() as i64;

        self.confidence = ClockConfidence::new(reference.accuracy_ns());
        self.last_sync = Some(now);
        self.synchronized = true;

        Ok(SyncReport {
            synced_at: now,
            offset_ns,
            confidence_after: self.confidence,
            successful: true,
        })
    }

    fn is_synchronized(&self) -> bool {
        self.synchronized && self.last_sync.is_some()
    }

    fn drift_rate(&self) -> DriftRate {
        self.drift_rate
    }
}

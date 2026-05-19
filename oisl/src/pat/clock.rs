// Precision Clock - for synchronized PAT acquisition
//
// GPS-disciplined oscillators provide ~50ns accuracy. Without GPS (denied
// environments), satellite clocks drift. The PAT coordinator needs to know
// clock confidence and reduce acquisition attempts when clocks are degraded.

use crate::TimeReference;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Precision timestamp with nanosecond precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PrecisionTimestamp {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl PrecisionTimestamp {
    pub fn new(seconds: i64, nanoseconds: u32) -> Self {
        Self {
            seconds,
            nanoseconds,
        }
    }

    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self {
            seconds: dt.timestamp(),
            nanoseconds: dt.timestamp_subsec_nanos(),
        }
    }

    pub fn to_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.seconds, self.nanoseconds).unwrap_or(Utc::now())
    }

    pub fn now() -> Self {
        Self::from_datetime(Utc::now())
    }

    pub fn duration_since(&self, other: &Self) -> Duration {
        let secs = self.seconds - other.seconds;
        let nanos = self.nanoseconds as i64 - other.nanoseconds as i64;
        let total_nanos = secs * 1_000_000_000 + nanos;
        Duration::nanoseconds(total_nanos)
    }
}

impl Default for PrecisionTimestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl From<DateTime<Utc>> for PrecisionTimestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Self::from_datetime(dt)
    }
}

/// Clock confidence - microseconds of uncertainty
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClockConfidence {
    pub uncertainty_ns: u64,
}

impl ClockConfidence {
    pub fn new(uncertainty_ns: u64) -> Self {
        Self { uncertainty_ns }
    }

    pub fn is_high_precision(&self) -> bool {
        self.uncertainty_ns < 1000 // < 1 microsecond
    }

    pub fn is_degraded(&self) -> bool {
        self.uncertainty_ns > 100_000 // > 100 microseconds
    }

    /// GPS-disciplined oscillator accuracy
    pub fn gps_disciplined() -> Self {
        Self::new(50) // 50 nanoseconds
    }

    /// Standard oscillator accuracy (drifts over time)
    pub fn standard_oscillator() -> Self {
        Self::new(10_000_000) // 10 milliseconds
    }
}

impl Default for ClockConfidence {
    fn default() -> Self {
        Self::gps_disciplined()
    }
}

/// Precision clock trait
pub trait PrecisionClock: Send + Sync {
    /// Get current precision timestamp
    fn now(&self) -> PrecisionTimestamp;

    /// Get clock confidence
    fn confidence(&self) -> ClockConfidence;

    /// Sync to time reference
    fn sync_to(&mut self, reference: &crate::pat::TimeReference) -> Result<SyncReport, ClockError>;

    /// Check if clock is synchronized
    fn is_synchronized(&self) -> bool;
}

/// Sync report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub synced_at: DateTime<Utc>,
    pub offset_ns: i64,
    pub confidence_after: ClockConfidence,
    pub successful: bool,
}

/// Clock error
#[derive(Debug, Clone, thiserror::Error)]
pub enum ClockError {
    #[error("Clock synchronization failed: {0}")]
    SyncFailed(String),

    #[error("Clock drift detected: {0} ns")]
    DriftDetected(i64),

    #[error("Clock not responding")]
    NotResponding,

    #[error("Invalid time reference")]
    InvalidReference,
}

/// Default precision clock implementation
pub struct DefaultPrecisionClock {
    confidence: ClockConfidence,
    last_sync: Option<DateTime<Utc>>,
    drift_rate_ns_per_sec: f64,
}

impl DefaultPrecisionClock {
    pub fn new() -> Self {
        Self {
            confidence: ClockConfidence::gps_disciplined(),
            last_sync: None,
            drift_rate_ns_per_sec: 0.1, // 0.1 ns/sec drift
        }
    }

    /// Create clock with specified confidence
    pub fn with_confidence(confidence: ClockConfidence) -> Self {
        Self {
            confidence,
            last_sync: None,
            drift_rate_ns_per_sec: 1.0,
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

    fn sync_to(&mut self, reference: &crate::pat::TimeReference) -> Result<SyncReport, ClockError> {
        let now = Utc::now();
        let offset_ns = reference.accuracy_ns as i64;

        self.confidence = ClockConfidence::new(reference.accuracy_ns);
        self.last_sync = Some(now);

        Ok(SyncReport {
            synced_at: now,
            offset_ns,
            confidence_after: self.confidence,
            successful: true,
        })
    }

    fn is_synchronized(&self) -> bool {
        if let Some(last_sync) = self.last_sync {
            let elapsed = Utc::now().signed_duration_since(last_sync);
            // Consider synchronized if synced within last hour
            elapsed < Duration::hours(1)
        } else {
            false
        }
    }
}

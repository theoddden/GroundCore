// PAT Coordination Plane - hard real-time synchronized acquisition
//
// Two terminals on two satellites must begin acquisition at synchronized
// timestamps with no real-time communication channel. They must agree on
// pointing vectors, frequency, modulation, and acquisition sequence ahead
// of time. If their clocks disagree by more than a few hundred microseconds,
// acquisition fails.

pub mod coordinator;
pub mod clock;

pub use coordinator::{PatCoordinator, ScheduledAcquisition, PatEvent, PatEventType};
pub use clock::{PrecisionClock, PrecisionTimestamp, ClockConfidence};

use crate::{TerminalId, AcquisitionId, OctConfiguration, PointingVector};
use crate::pat::coordinator::{AcquisitionSequence, FallbackAction};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

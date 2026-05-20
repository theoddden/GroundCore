// PAT Coordination Plane - hard real-time synchronized acquisition
//
// Two terminals on two satellites must begin acquisition at synchronized
// timestamps with no real-time communication channel. They must agree on
// pointing vectors, frequency, modulation, and acquisition sequence ahead
// of time. If their clocks disagree by more than a few hundred microseconds,
// acquisition fails.

pub mod clock;
pub mod coordinator;

pub use clock::{ClockConfidence, PrecisionClock, PrecisionTimestamp};
pub use coordinator::{PatCoordinator, PatEvent, PatEventType, ScheduledAcquisition};

use crate::pat::coordinator::{AcquisitionSequence, FallbackAction};
use crate::{AcquisitionId, OctConfiguration, PointingVector, TerminalId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

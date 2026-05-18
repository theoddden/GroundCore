// Physical Plane - vendor abstraction over shared SDA OCT protocol
//
// Mynaric and Tesat implement the same SDA OCT Standard at the optical layer.
// They differ at the management interface - how you configure the terminal,
// monitor its health, command it to point. The physical plane abstracts the
// management interface, not the optical layer.

pub mod terminal;
pub mod configuration;
pub mod vendors;

pub use terminal::{OpticalTerminal, EthernetEndpoint, TelemetryStream, PatHandle};
pub use configuration::{
    OctConfiguration, Modulation, FecConfiguration, FecCode, LinkType,
    OctStandardVersion, ArqConfiguration, LdpcVariant, CodeRate, BaudRate,
};
pub use vendors::{CondorMk3, Scot80, Vendor, SerialNumber};

use crate::{TerminalId, DataRate, Frequency, Bytes};
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::time::Instant;

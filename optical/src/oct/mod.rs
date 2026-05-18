// SDA OCT Standard implementation
//
// The SDA OCT Standard is well-defined and the implementation tracks the spec closely.

pub mod standard;
pub mod config;
pub mod negotiation;
pub mod framing;

pub use standard::{OctStandardVersion, OctStandard};
pub use config::{
    OctConfiguration, LinkType, Modulation, FecConfiguration,
    FecCode, ArqConfiguration, TrackingTone, AtmosphericModel,
};
pub use negotiation::{negotiate_version, NegotiationError};
pub use framing::{EthernetFrame, FramingError};

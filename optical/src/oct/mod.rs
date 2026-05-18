// SDA OCT Standard implementation
//
// The SDA OCT Standard is well-defined and the implementation tracks the spec closely.

pub mod config;
pub mod framing;
pub mod negotiation;
pub mod standard;

pub use config::{
    ArqConfiguration, AtmosphericModel, FecCode, FecConfiguration, LinkType, Modulation,
    OctConfiguration, TrackingTone,
};
pub use framing::{EthernetFrame, FramingError};
pub use negotiation::{NegotiationError, negotiate_version};
pub use standard::{OctStandard, OctStandardVersion};

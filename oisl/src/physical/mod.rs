// Physical Plane - vendor abstraction over shared SDA OCT protocol
//
// Mynaric and Tesat implement the same SDA OCT Standard at the optical layer.
// They differ at the management interface - how you configure the terminal,
// monitor its health, command it to point. The physical plane abstracts the
// management interface, not the optical layer.

pub mod configuration;
pub mod terminal;
pub mod vendors;

pub use configuration::{
    ArqConfiguration, BaudRate, CodeRate, FecCode, FecConfiguration, LdpcVariant, LinkType,
    Modulation, OctConfiguration, OctStandardVersion,
};
pub use terminal::{EthernetEndpoint, OpticalTerminal, PatHandle, TelemetryStream};
pub use vendors::{CondorMk3, Scot80, SerialNumber, Vendor};


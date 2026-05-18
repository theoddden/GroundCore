//! Spectrum policy compiler
//!
//! This compiler takes regulatory rules (FCC Part 5, ITU Radio Regulations,
//! country-specific telecom laws) and emits Rust types that statically enforce
//! compliance at compile time.
//!
//! The key insight: a function that transmits on a particular radio band
//! requires a license value as one of its parameters, and the license is typed
//! to that specific band. If you don't have the license, your code doesn't compile.

pub mod parser;
pub mod generator;
pub mod rules;

pub use parser::PolicyParser;
pub use generator::GeneratedTypes;
pub use rules::{FrequencyBand, PowerLimit, LicenseRequirement, CoordinationRequirement};

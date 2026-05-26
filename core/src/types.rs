//! Core types for Ground Station Core

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Link identifier (UUID-typed — opaque, not a String alias)
pub type LinkId = Uuid;

/// Numeric quantities
pub type Frequency = u64;
pub type DataRate = u64;
pub type Bytes = u64;

/// Generate a strongly-typed string-ID newtype with full trait bounds.
///
/// Implements: `From<String>`, `From<&str>`, `Deref<Target=str>`, `Display`,
/// `AsRef<str>`, `Borrow<str>`, `PartialEq<str>`, `PartialEq<String>`,
/// `Hash`, `Serialize/Deserialize` (transparent), `Default`, `Clone`, `Debug`.
macro_rules! string_id_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl From<&String> for $name {
            fn from(s: &String) -> Self {
                Self(s.clone())
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<String> for $name {
            fn eq(&self, other: &String) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<$name> for str {
            fn eq(&self, other: &$name) -> bool {
                self == other.0.as_str()
            }
        }

        impl PartialEq<$name> for String {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0.as_str()
            }
        }

        impl AsRef<std::ffi::OsStr> for $name {
            fn as_ref(&self) -> &std::ffi::OsStr {
                std::ffi::OsStr::new(&self.0)
            }
        }
    };
}

string_id_newtype!(SatelliteId, "Strongly-typed satellite identifier.");
string_id_newtype!(StationId, "Strongly-typed ground-station identifier.");
string_id_newtype!(PassId, "Strongly-typed pass identifier.");
string_id_newtype!(CustomerId, "Strongly-typed customer identifier.");

/// Frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrequencyBand {
    LBand,
    SBand,
    CBand,
    XBand,
    KuBand,
    KaBand,
}

impl FrequencyBand {
    pub fn name(&self) -> &str {
        match self {
            FrequencyBand::LBand => "L-band",
            FrequencyBand::SBand => "S-band",
            FrequencyBand::CBand => "C-band",
            FrequencyBand::XBand => "X-band",
            FrequencyBand::KuBand => "Ku-band",
            FrequencyBand::KaBand => "Ka-band",
        }
    }
}

//! Bi-temporal timestamp types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Event time — when the satellite emitted the signal
/// Computed from the satellite's orbital position at emission time
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventTime(DateTime<Utc>);

impl EventTime {
    pub fn new(time: DateTime<Utc>) -> Self {
        Self(time)
    }
    
    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
}

impl From<DateTime<Utc>> for EventTime {
    fn from(time: DateTime<Utc>) -> Self {
        Self(time)
    }
}

/// Reception time — when the ground station received the signal
/// Actual wall-clock time when the sample was processed
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReceptionTime(DateTime<Utc>);

impl ReceptionTime {
    pub fn new(time: DateTime<Utc>) -> Self {
        Self(time)
    }
    
    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
    
    pub fn now() -> Self {
        Self(Utc::now())
    }
}

impl From<DateTime<Utc>> for ReceptionTime {
    fn from(time: DateTime<Utc>) -> Self {
        Self(time)
    }
}

/// Bi-temporal timestamp pair
/// Combines event time and reception time for provable logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BiTemporal<T> {
    /// The value being timestamped
    pub value: T,
    /// When the event occurred (satellite emission time)
    pub event_time: EventTime,
    /// When the event was received (ground station reception time)
    pub reception_time: ReceptionTime,
}

impl<T> BiTemporal<T> {
    pub fn new(value: T, event_time: EventTime, reception_time: ReceptionTime) -> Self {
        Self {
            value,
            event_time,
            reception_time,
        }
    }
    
    /// Create a bi-temporal value with current reception time
    pub fn with_current_reception(value: T, event_time: EventTime) -> Self {
        Self {
            value,
            event_time,
            reception_time: ReceptionTime::now(),
        }
    }
    
    /// Map over the value while preserving timestamps
    pub fn map<U, F>(self, f: F) -> BiTemporal<U>
    where
        F: FnOnce(T) -> U,
    {
        BiTemporal {
            value: f(self.value),
            event_time: self.event_time,
            reception_time: self.reception_time,
        }
    }
    
    /// Get the time difference between event and reception
    pub fn propagation_delay(&self) -> chrono::Duration {
        self.reception_time.as_datetime() - self.event_time.as_datetime()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bitemporal_creation() {
        let event = EventTime::new(Utc::now() - chrono::Duration::seconds(1));
        let reception = ReceptionTime::now();
        
        let bt: BiTemporal<i32> = BiTemporal::new(42, event, reception);
        
        assert_eq!(bt.value, 42);
        assert!(bt.propagation_delay().num_seconds() > 0);
    }
    
    #[test]
    fn test_bitemporal_map() {
        let event = EventTime::new(Utc::now() - chrono::Duration::seconds(1));
        let reception = ReceptionTime::now();
        
        let bt1: BiTemporal<i32> = BiTemporal::new(42, event, reception);
        let bt2: BiTemporal<String> = bt1.map(|x| x.to_string());
        
        assert_eq!(bt2.value, "42");
        assert_eq!(bt2.event_time, event);
        assert_eq!(bt2.reception_time, reception);
    }
}

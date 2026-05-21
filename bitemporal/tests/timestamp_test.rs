#[cfg(test)]
mod tests {
    use bitemporal::{BiTemporal, EventTime, ReceptionTime};
    use chrono::{DateTime, Utc};

    #[test]
    fn test_bitemporal_creation() {
        let event_time = DateTime::<Utc>::from_timestamp(1234567890, 0).unwrap();
        let reception_time = DateTime::<Utc>::from_timestamp(1234567895, 0).unwrap();

        let bi_temporal = BiTemporal::new(
            event_time,
            EventTime::new(event_time),
            ReceptionTime::new(reception_time),
        );

        assert_eq!(bi_temporal.event_time.as_datetime(), event_time);
        assert_eq!(bi_temporal.reception_time.as_datetime(), reception_time);
    }

    #[test]
    fn test_event_time_creation() {
        let dt = DateTime::<Utc>::from_timestamp(1234567890, 0).unwrap();
        let event_time = EventTime::new(dt);
        assert_eq!(event_time.as_datetime(), dt);
    }

    #[test]
    fn test_reception_time_creation() {
        let dt = DateTime::<Utc>::from_timestamp(1234567890, 0).unwrap();
        let reception_time = ReceptionTime::new(dt);
        assert_eq!(reception_time.as_datetime(), dt);
    }

    #[test]
    fn test_bitemporal_serialization() {
        let event_time = DateTime::<Utc>::from_timestamp(1234567890, 0).unwrap();
        let reception_time = DateTime::<Utc>::from_timestamp(1234567895, 0).unwrap();

        let bi_temporal = BiTemporal::new(
            event_time,
            EventTime::new(event_time),
            ReceptionTime::new(reception_time),
        );

        let serialized = serde_json::to_string(&bi_temporal).unwrap();
        let deserialized: BiTemporal<DateTime<Utc>> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.event_time.as_datetime(), event_time);
        assert_eq!(deserialized.reception_time.as_datetime(), reception_time);
    }
}

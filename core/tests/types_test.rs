#[cfg(test)]
mod tests {
    use ground_core::types::{Bytes, DataRate, Frequency, FrequencyBand};

    #[test]
    fn test_frequency_band_names() {
        assert_eq!(FrequencyBand::LBand.name(), "L-band");
        assert_eq!(FrequencyBand::SBand.name(), "S-band");
        assert_eq!(FrequencyBand::CBand.name(), "C-band");
        assert_eq!(FrequencyBand::XBand.name(), "X-band");
        assert_eq!(FrequencyBand::KuBand.name(), "Ku-band");
        assert_eq!(FrequencyBand::KaBand.name(), "Ka-band");
    }

    #[test]
    fn test_data_rate_ord() {
        let rate1 = DataRate(1_000_000);
        let rate2 = DataRate(2_000_000);
        assert!(rate1 < rate2);
        assert!(rate2 > rate1);
        assert_eq!(rate1, DataRate(1_000_000));
    }

    #[test]
    fn test_frequency_values() {
        let freq1: Frequency = 1_000_000;
        let freq2: Frequency = 2_000_000;

        assert_eq!(freq1, 1_000_000);
        assert_eq!(freq2, 2_000_000);
        assert_ne!(freq1, freq2);
    }

    #[test]
    fn test_bytes_ord() {
        let bytes1 = Bytes(1024);
        let bytes2 = Bytes(2048);
        assert!(bytes1 < bytes2);
        assert!(bytes2 > bytes1);
        assert_eq!(bytes1, Bytes(1024));
    }

    #[test]
    fn test_frequency_band_equality() {
        assert_eq!(FrequencyBand::LBand, FrequencyBand::LBand);
        assert_ne!(FrequencyBand::LBand, FrequencyBand::SBand);
    }
}

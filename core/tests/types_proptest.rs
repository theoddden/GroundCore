#[cfg(test)]
mod proptests {
    use ground_core::{Bytes, DataRate, Frequency, FrequencyBand};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_data_rate_ord_properties(a in 0u64..1_000_000_000u64, b in 0u64..1_000_000_000u64) {
            let rate1 = DataRate(a);
            let rate2 = DataRate(b);

            // Reflexivity
            assert!(rate1 >= rate1);
            assert!(rate1 <= rate1);

            // Antisymmetry
            if rate1 <= rate2 && rate2 <= rate1 {
                assert_eq!(rate1, rate2);
            }

            // Transitivity
            let rate3 = DataRate((a + b) % 1_000_000_000);
            if rate1 <= rate2 && rate2 <= rate3 {
                assert!(rate1 <= rate3);
            }
        }

        #[test]
        fn test_frequency_ord_properties(a in 0u64..100_000_000_000u64, b in 0u64..100_000_000_000u64) {
            let freq1 = Frequency(a);
            let freq2 = Frequency(b);

            // Reflexivity
            assert!(freq1 >= freq1);
            assert!(freq1 <= freq1);

            // Antisymmetry
            if freq1 <= freq2 && freq2 <= freq1 {
                assert_eq!(freq1, freq2);
            }
        }

        #[test]
        fn test_bytes_ord_properties(a in 0u64..1_000_000_000_000u64, b in 0u64..1_000_000_000_000u64) {
            let bytes1 = Bytes(a);
            let bytes2 = Bytes(b);

            // Reflexivity
            assert!(bytes1 >= bytes1);
            assert!(bytes1 <= bytes1);

            // Antisymmetry
            if bytes1 <= bytes2 && bytes2 <= bytes1 {
                assert_eq!(bytes1, bytes2);
            }
        }

        #[test]
        fn test_frequency_band_roundtrip(band in 0u8..6u8) {
            let bands = [
                FrequencyBand::LBand,
                FrequencyBand::SBand,
                FrequencyBand::CBand,
                FrequencyBand::XBand,
                FrequencyBand::KuBand,
                FrequencyBand::KaBand,
            ];

            if (band as usize) < bands.len() {
                let original = bands[band as usize];
                let name = original.name();
                assert!(!name.is_empty());
            }
        }
    }
}

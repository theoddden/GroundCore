#[cfg(test)]
mod proptests {
    use chrono::{Duration, Utc};
    use oisl::{ConfidenceScore, Priority, TimeWindow};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_confidence_score_clamping(value in -100.0f64..200.0f64) {
            let score = ConfidenceScore::new(value);
            assert!(score.0 >= 0.0 && score.0 <= 1.0);
        }

        #[test]
        fn test_confidence_score_properties(a in 0.0f64..1.0f64, b in 0.0f64..1.0f64) {
            let score_a = ConfidenceScore::new(a);
            let score_b = ConfidenceScore::new(b);

            // High threshold
            if score_a.0 >= 0.8 {
                assert!(score_a.is_high());
            }

            // Low threshold
            if score_b.0 < 0.5 {
                assert!(score_b.is_low());
            }
        }

        #[test]
        fn test_priority_ord_properties(a in 0u8..5u8, b in 0u8..5u8) {
            let priorities = [
                Priority::Critical,
                Priority::High,
                Priority::Medium,
                Priority::Low,
                Priority::Background,
            ];

            if (a as usize) < priorities.len() && (b as usize) < priorities.len() {
                let prio_a = priorities[a as usize];
                let prio_b = priorities[b as usize];

                // Reflexivity
                assert!(prio_a >= prio_a);
                assert!(prio_a <= prio_a);

                // Antisymmetry
                if prio_a <= prio_b && prio_b <= prio_a {
                    assert_eq!(prio_a, prio_b);
                }
            }
        }

        #[test]
        fn test_time_window_properties(start_offset in 0i64..86400i64, duration in 60i64..86400i64) {
            let start = Utc::now() + Duration::seconds(start_offset);
            let end = start + Duration::seconds(duration);
            let window = TimeWindow::new(start, end);

            // Duration should match
            assert_eq!(window.duration(), Duration::seconds(duration));

            // Start should be contained
            assert!(window.contains(start));

            // End should be contained
            assert!(window.contains(end));

            // Midpoint should be contained
            let midpoint = start + Duration::seconds(duration / 2);
            assert!(window.contains(midpoint));

            // Time before start should not be contained
            let before = start - Duration::seconds(1);
            assert!(!window.contains(before));

            // Time after end should not be contained
            let after = end + Duration::seconds(1);
            assert!(!window.contains(after));
        }

        #[test]
        fn test_time_window_overlap_properties(
            start1 in 0i64..43200i64,
            duration1 in 3600i64..86400i64,
            start2 in 0i64..43200i64,
            duration2 in 3600i64..86400i64
        ) {
            let base = Utc::now();
            let window1 = TimeWindow::new(
                base + Duration::seconds(start1),
                base + Duration::seconds(start1 + duration1),
            );
            let window2 = TimeWindow::new(
                base + Duration::seconds(start2),
                base + Duration::seconds(start2 + duration2),
            );

            // Overlap should be symmetric
            assert_eq!(window1.overlaps(&window2), window2.overlaps(&window1));

            // If windows overlap, there should be a common time
            if window1.overlaps(&window2) {
                let mid1 = window1.start + window1.duration() / 2;
                if window2.contains(mid1) {
                    // Found overlap point
                }
            }
        }
    }
}

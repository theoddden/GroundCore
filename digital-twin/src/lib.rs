//! Aalyria's Spacetime answers what should happen — it's a planning engine. This twin answers
//! where reality diverges from physics, by how much, when we first knew, and what likely caused it —
//! and signs that record bi-temporally so it survives an incident review, an insurance dispute, or
//! a regulator's audit. We do not out-simulate Aalyria at simulation. We reconcile, and we make the
//! reconciliation auditable. That capability requires a bi-temporal substrate they don't have and we
//! already built.
//!
//! # Digital Twin Architecture
//!
//! The digital twin is a physics-accurate simulation layer that runs alongside the pass shard,
//! synchronized in real-time with actual link telemetry. It enables:
//!
//! - **Pre-failure warning**: Predict 60 seconds ahead to reroute before degradation
//! - **Shadow execution**: Test scheduling heuristics safely against the twin
//! - **Counterfactual analysis**: Replay what-we-knew-when for incident review
//! - **Hardware-free CI/CD**: Test scenarios against realistic physics in CI
//! - **Operator situational awareness**: Query the twin for physics-grounded answers
//!
//! # Core Primitive
//!
//! Divergence from physics prediction is itself the anomaly signal — no trained model, no
//! hand-tuned thresholds. Two things make this rigorous:
//!
//! 1. **Predict bounded intervals, not points.** Given TLE staleness and atmospheric variance,
//!    the expected metric is X ± [guaranteed interval]. An observation outside the interval
//!    is a provable anomaly, not a probabilistic guess. This is set-membership estimation.
//!
//! 2. **Detect when the regime shifted, not just that one sample is high.** Use CUSUM
//!    change-point detection on the divergence stream. This is decades-old, battle-tested
//!    signal-processing literature.
//!
//! # ECS Architecture
//!
//! The twin uses Bevy ECS for simulation:
//! - **Entity** = satellite or ground station
//! - **Component** = orbital state, optical terminal config, link budget, etc.
//! - **System** = SGP4 propagation, link visibility calculation, Doppler prediction, etc.
//!
//! Systems run in parallel across all entities using Rayon for cache-efficient access.
//!
//! # Bi-Temporal Logging
//!
//! Every divergence is logged with two timestamps:
//! - **Event time**: when the physics model predicted the metric
//! - **Reception time**: when the system observed the metric
//!
//! This bi-temporal logging makes everything provable later — you can prove exactly what
//! you received and exactly when. It's critical for incident analysis, insurance disputes,
//! and regulatory audits.

pub mod components;
pub mod divergence;
pub mod runtime;
pub mod snapshot;
pub mod systems;

// Re-export key types for convenience
pub use components::*;
pub use divergence::{CusumDetector, DivergenceDetector, Interval, MetricDivergence, MetricType};
pub use runtime::{
    AnomalySignal, DigitalTwinRuntime, ObservedMetric, TwinConfig, TwinForecast, TwinTickSchedule,
    create_twin_channels,
};
pub use snapshot::{CounterfactualQuery, SnapshotManager, TwinSnapshot, WorldSummary};
pub use systems::{compute_link_intervals, propagate_orbits, visibility_windows};

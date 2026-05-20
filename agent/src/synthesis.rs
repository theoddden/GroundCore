//! Anomaly synthesis from weak signals
//!
//! The agent runs cheap statistical anomaly detection across all telemetry,
//! producing thousands of weak signals per day. Most are noise.
//! The agent synthesizes these into coherent narratives rather than alerting
//! on each individual signal.

use crate::observation::SystemState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Weak signal from statistical anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeakSignal {
    /// Signal ID
    pub signal_id: String,
    /// When this signal was detected
    pub detected_at: DateTime<Utc>,
    /// Signal type
    pub signal_type: SignalType,
    /// Component that generated this signal
    pub component: String,
    /// Signal strength (z-score or similar)
    pub strength: f64,
    /// Raw value that triggered the signal
    pub raw_value: f64,
    /// Expected value
    pub expected_value: f64,
    /// Contextual data
    pub context: serde_json::Value,
}

/// Type of weak signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    /// Statistical anomaly (z-score threshold)
    StatisticalAnomaly { z_score: f64 },
    /// Change point detected
    ChangePoint,
    /// Autocorrelation break
    AutocorrelationBreak,
    /// Rate anomaly
    RateAnomaly,
    /// Pattern deviation
    PatternDeviation,
}

/// Synthesized anomaly alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    /// Alert ID
    pub alert_id: String,
    /// When this alert was generated
    pub generated_at: DateTime<Utc>,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert title
    pub title: String,
    /// Detailed explanation
    pub explanation: String,
    /// Constituent weak signals
    pub signals: Vec<WeakSignal>,
    /// Recommended actions
    pub recommendations: Vec<String>,
    /// Evidence chain
    pub evidence: EvidenceChain,
}

/// Alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Evidence chain for the alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChain {
    /// Historical patterns
    pub historical_patterns: Vec<String>,
    /// Correlated events
    pub correlated_events: Vec<String>,
    /// Similar past incidents
    pub similar_incidents: Vec<String>,
}

/// Maximum signals buffered before oldest are dropped to prevent OOM under cascade failures
const MAX_SIGNAL_BUFFER: usize = 4096;

/// Anomaly synthesizer
pub struct AnomalySynthesizer {
    /// Bounded ring buffer — oldest signals dropped under signal flood
    signal_buffer: VecDeque<WeakSignal>,
    /// Maximum buffer capacity
    max_buffer_size: usize,
    /// Synthesis interval (seconds)
    synthesis_interval_sec: u64,
    /// Last synthesis time
    last_synthesis: Option<DateTime<Utc>>,
    /// Signals dropped due to buffer overflow (for telemetry)
    dropped_signals: u64,
}

impl AnomalySynthesizer {
    pub fn new(synthesis_interval_sec: u64) -> Self {
        Self::with_capacity(synthesis_interval_sec, MAX_SIGNAL_BUFFER)
    }

    pub fn with_capacity(synthesis_interval_sec: u64, max_buffer_size: usize) -> Self {
        Self {
            signal_buffer: VecDeque::with_capacity(max_buffer_size),
            max_buffer_size,
            synthesis_interval_sec,
            last_synthesis: None,
            dropped_signals: 0,
        }
    }

    /// Add a weak signal. Drops the oldest signal if the buffer is full.
    pub fn add_signal(&mut self, signal: WeakSignal) {
        if self.signal_buffer.len() >= self.max_buffer_size {
            self.signal_buffer.pop_front();
            self.dropped_signals += 1;
            tracing::warn!(
                "Signal buffer full — dropped oldest signal (total dropped: {})",
                self.dropped_signals
            );
        }
        self.signal_buffer.push_back(signal);
    }

    /// Number of signals dropped due to buffer overflow
    pub fn dropped_signals(&self) -> u64 {
        self.dropped_signals
    }

    /// Check if synthesis should run
    pub fn should_synthesize(&self) -> bool {
        if let Some(last) = self.last_synthesis {
            let elapsed = (Utc::now() - last).num_seconds();
            elapsed >= self.synthesis_interval_sec as i64
        } else {
            true
        }
    }

    /// Synthesize weak signals into coherent alerts
    pub fn synthesize(&mut self, system_state: &SystemState) -> Vec<AnomalyAlert> {
        let now = Utc::now();
        self.last_synthesis = Some(now);

        let mut alerts = Vec::new();

        // Group signals by component
        let mut by_component: std::collections::HashMap<String, Vec<WeakSignal>> =
            std::collections::HashMap::new();

        for signal in &self.signal_buffer {
            by_component
                .entry(signal.component.clone())
                .or_default()
                .push(signal.clone());
        }

        // Synthesize alerts for each component
        for (component, signals) in by_component {
            if signals.len() >= 3 {
                // Multiple signals from same component - likely coherent issue
                let alert = self.synthesize_component_alert(component, signals, system_state);
                alerts.push(alert);
            }
        }

        // Clear buffer after synthesis
        self.signal_buffer.clear();

        alerts
    }

    /// Synthesize alert for a specific component
    fn synthesize_component_alert(
        &self,
        component: String,
        signals: Vec<WeakSignal>,
        _system_state: &SystemState,
    ) -> AnomalyAlert {
        let max_strength = signals
            .iter()
            .map(|s| s.strength.abs())
            .fold(0.0_f64, f64::max);

        let severity = if max_strength > 5.0 {
            AlertSeverity::Critical
        } else if max_strength > 3.0 {
            AlertSeverity::High
        } else if max_strength > 2.0 {
            AlertSeverity::Medium
        } else {
            AlertSeverity::Low
        };

        let title = format!("Anomaly detected in {}", component);

        let explanation = format!(
            "Detected {} weak signals from {} with maximum strength {:.2}. Pattern suggests {}.",
            signals.len(),
            component,
            max_strength,
            self.infer_pattern(&signals)
        );

        let recommendations = self.generate_recommendations(&signals);

        AnomalyAlert {
            alert_id: uuid::Uuid::new_v4().to_string(),
            generated_at: Utc::now(),
            severity,
            title,
            explanation,
            signals,
            recommendations,
            evidence: EvidenceChain {
                historical_patterns: Vec::new(),
                correlated_events: Vec::new(),
                similar_incidents: Vec::new(),
            },
        }
    }

    /// Infer pattern from signals
    fn infer_pattern(&self, signals: &[WeakSignal]) -> &str {
        // Simple pattern inference based on signal types
        let has_statistical = signals
            .iter()
            .any(|s| matches!(s.signal_type, SignalType::StatisticalAnomaly { .. }));
        let has_change_point = signals
            .iter()
            .any(|s| matches!(s.signal_type, SignalType::ChangePoint));

        if has_change_point {
            "sudden state change"
        } else if has_statistical {
            "statistical deviation from baseline"
        } else {
            "multi-factor anomaly"
        }
    }

    /// Generate recommendations based on signals
    fn generate_recommendations(&self, signals: &[WeakSignal]) -> Vec<String> {
        let mut recommendations = Vec::new();

        for signal in signals {
            match signal.signal_type {
                SignalType::StatisticalAnomaly { z_score } if z_score > 4.0 => {
                    recommendations.push(format!(
                        "Investigate {} - extreme statistical anomaly detected",
                        signal.component
                    ));
                }
                SignalType::ChangePoint => {
                    recommendations.push(format!(
                        "Check {} configuration - change point detected",
                        signal.component
                    ));
                }
                _ => {}
            }
        }

        if recommendations.is_empty() {
            recommendations.push("Monitor for further signals".to_string());
        }

        recommendations
    }
}

// Link metrics and history

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Metric snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub timestamp: DateTime<Utc>,
    pub data_rate_actual: u64,
    pub bit_error_rate: f64,
    pub signal_quality_db: f64,
    pub pointing_error_urad: f64,
}

impl MetricSnapshot {
    pub fn new(
        data_rate_actual: u64,
        bit_error_rate: f64,
        signal_quality_db: f64,
        pointing_error_urad: f64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            data_rate_actual,
            bit_error_rate,
            signal_quality_db,
            pointing_error_urad,
        }
    }
}

/// Bounded history for metrics
#[derive(Debug, Clone)]
pub struct BoundedHistory<T> {
    history: VecDeque<T>,
    max_size: usize,
}

impl<T> BoundedHistory<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(item);
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    pub fn latest(&self) -> Option<&T> {
        self.history.back()
    }

    pub fn oldest(&self) -> Option<&T> {
        self.history.front()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.history.iter()
    }

    pub fn average_ber(&self) -> f64
    where
        T: AsRef<MetricSnapshot>,
    {
        if self.history.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = self.history.iter()
            .map(|s| s.as_ref().bit_error_rate)
            .sum();
        
        sum / self.history.len() as f64
    }

    pub fn trend(&self) -> MetricTrend
    where
        T: AsRef<MetricSnapshot>,
    {
        if self.history.len() < 2 {
            return MetricTrend::Stable;
        }

        let recent = self.history.iter().rev().take(5).collect::<Vec<_>>();
        let recent_avg: f64 = recent.iter()
            .map(|s| s.as_ref().signal_quality_db)
            .sum::<f64>() / recent.len() as f64;

        let older = self.history.iter().take(5).collect::<Vec<_>>();
        let older_avg: f64 = older.iter()
            .map(|s| s.as_ref().signal_quality_db)
            .sum::<f64>() / older.len() as f64;

        let delta = recent_avg - older_avg;
        
        if delta > 1.0 {
            MetricTrend::Improving
        } else if delta < -1.0 {
            MetricTrend::Degrading
        } else {
            MetricTrend::Stable
        }
    }
}

/// Metric trend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricTrend {
    Improving,
    Stable,
    Degrading,
}

//! Two-Line Element (TLE) data management

use batching::TleBatcher;
use chrono::{DateTime, Utc};
use ground_core::{Result, SatelliteId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TLE data for a satellite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TleData {
    /// Satellite identifier
    pub satellite_id: SatelliteId,
    /// TLE line 1
    pub line1: String,
    /// TLE line 2
    pub line2: String,
    /// Epoch of this TLE
    pub epoch: DateTime<Utc>,
    /// Source of this TLE (e.g., CelesTrak, Space-Track)
    pub source: String,
}

impl TleData {
    /// Parse TLE from standard two-line format
    pub fn from_lines(
        satellite_id: SatelliteId,
        line1: &str,
        line2: &str,
        source: String,
    ) -> Result<Self> {
        // Validate TLE format
        if !line1.starts_with('1') || !line2.starts_with('2') {
            return Err(ground_core::GroundStationError::Tracking(
                "Invalid TLE format".to_string(),
            ));
        }

        // Extract epoch from line1 (columns 19-32)
        let epoch_str = &line1[18..32];
        let epoch = parse_tle_epoch(epoch_str)?;

        Ok(Self {
            satellite_id,
            line1: line1.to_string(),
            line2: line2.to_string(),
            epoch,
            source,
        })
    }

    /// Get the satellite catalog number from the TLE
    pub fn catalog_number(&self) -> Result<u32> {
        let num_str = &self.line1[2..7];
        num_str.trim().parse().map_err(|_| {
            ground_core::GroundStationError::Tracking("Invalid catalog number".to_string())
        })
    }
}

/// Parse TLE epoch string (YYDDD.DDDDDDDD format)
fn parse_tle_epoch(s: &str) -> Result<DateTime<Utc>> {
    // TLE epoch format: YYDDD.DDDDDDDD
    // YY: last two digits of year
    // DDD: day of year
    // .DDDDDDDD: fractional day

    let year_part: u32 = s[0..2]
        .parse()
        .map_err(|_| ground_core::GroundStationError::Tracking("Invalid epoch year".to_string()))?;

    let day_part: f64 = s[2..]
        .parse()
        .map_err(|_| ground_core::GroundStationError::Tracking("Invalid epoch day".to_string()))?;

    // Convert YY to full year (assuming 1957-2056 range for SGP4)
    let year = if year_part > 56 {
        1900 + year_part
    } else {
        2000 + year_part
    };

    // Calculate date from day of year
    let start_of_year = chrono::NaiveDate::from_ymd_opt(year as i32, 1, 1)
        .ok_or_else(|| ground_core::GroundStationError::Tracking("Invalid year".to_string()))?;

    let days = day_part.floor() as u32;
    let fractional_day = day_part - days as f64;
    let seconds = fractional_day * 86400.0;

    let date = start_of_year + chrono::Duration::days(days as i64 - 1);
    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ground_core::GroundStationError::Tracking("Invalid date".to_string()))?
        + chrono::Duration::seconds(seconds as i64);

    Ok(DateTime::from_naive_utc_and_offset(datetime, Utc))
}

/// Set of TLEs for multiple satellites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TleSet {
    /// TLEs indexed by satellite ID
    pub tles: HashMap<SatelliteId, TleData>,
    /// When this set was fetched
    pub fetched_at: DateTime<Utc>,
}

impl TleSet {
    pub fn new() -> Self {
        Self {
            tles: HashMap::new(),
            fetched_at: Utc::now(),
        }
    }

    /// Add a TLE to the set
    pub fn add(&mut self, tle: TleData) {
        self.tles.insert(tle.satellite_id.clone(), tle);
    }

    /// Get TLE for a satellite
    pub fn get(&self, satellite_id: &SatelliteId) -> Option<&TleData> {
        self.tles.get(satellite_id)
    }

    /// Get all satellite IDs
    pub fn satellite_ids(&self) -> Vec<SatelliteId> {
        self.tles.keys().cloned().collect()
    }

    /// Remove a TLE
    pub fn remove(&mut self, satellite_id: &SatelliteId) -> Option<TleData> {
        self.tles.remove(satellite_id)
    }

    /// Check if a TLE needs refresh (older than threshold)
    pub fn needs_refresh(&self, satellite_id: &SatelliteId, max_age_hours: i64) -> bool {
        if let Some(tle) = self.get(satellite_id) {
            let age = Utc::now() - tle.epoch;
            age.num_hours() > max_age_hours
        } else {
            true
        }
    }

    /// Batch refresh multiple TLEs atomically using TleBatcher
    /// This ensures consistent constellation state at a known instant
    pub fn batch_refresh<F>(&mut self, fetch_fn: F, batch_size: usize) -> Result<()>
    where
        F: Fn(&SatelliteId) -> Result<TleData>,
    {
        let mut batcher = TleBatcher::new(batch_size);

        // Collect TLEs that need refresh
        let satellites_to_refresh: Vec<SatelliteId> = self.satellite_ids();

        for satellite_id in &satellites_to_refresh {
            match fetch_fn(satellite_id) {
                Ok(tle) => {
                    // Convert to batching crate's TleData format
                    let tle_data = batching::TleData {
                        line1: tle.line1.clone(),
                        line2: tle.line2.clone(),
                        epoch: tle.epoch,
                    };
                    batcher.add_tle(satellite_id.clone(), tle_data);

                    // Flush if batch is ready
                    if batcher.is_ready() {
                        if let Some(batch) = batcher.flush() {
                            self.apply_batch(batch);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch TLE for {}: {}", satellite_id, e);
                }
            }
        }

        // Force flush any remaining TLEs
        if let Some(batch) = batcher.force_flush() {
            self.apply_batch(batch);
        }

        self.fetched_at = Utc::now();
        Ok(())
    }

    /// Apply a batch of TLEs atomically
    fn apply_batch(&mut self, batch: batching::TleBatch) {
        let tle_count = batch.tles.len();
        let snapshot_time = batch.snapshot_time;

        for (satellite_id, tle_data) in batch.tles {
            let tle = TleData {
                satellite_id: satellite_id.clone(),
                line1: tle_data.line1,
                line2: tle_data.line2,
                epoch: tle_data.epoch,
                source: "batch_refresh".to_string(),
            };
            self.tles.insert(satellite_id, tle);
        }

        tracing::info!(
            "Applied TLE batch with {} satellites, snapshot time: {}",
            tle_count,
            snapshot_time
        );
    }
}

/// Batch TLE refresh for multiple satellites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TleRefreshBatch {
    /// Group ID for this batch
    pub group_id: String,
    /// When this batch was fetched
    pub fetched_at: DateTime<Utc>,
    /// TLEs in this batch
    pub tles: Vec<TleData>,
    /// Cross-validation result
    pub cross_validation: ValidationResult,
}

/// Result of cross-validating TLEs against previous epoch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Number of TLEs validated
    pub validated_count: usize,
    /// Number of TLEs with significant position drift
    pub drift_detected: usize,
    /// Maximum position drift in meters
    pub max_drift_meters: f64,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            validated_count: 0,
            drift_detected: 0,
            max_drift_meters: 0.0,
        }
    }
}

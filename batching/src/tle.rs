//! TLE batching for atomic constellation state updates

use chrono::{DateTime, Utc};
use ground_core::SatelliteId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TLE batch for atomic refresh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TleBatch {
    /// Batch ID
    pub batch_id: String,
    /// When this batch was created
    pub created_at: DateTime<Utc>,
    /// TLE data for each satellite
    pub tles: HashMap<SatelliteId, TleData>,
    /// Batch snapshot time (consistent constellation state)
    pub snapshot_time: DateTime<Utc>,
}

/// TLE data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TleData {
    pub line1: String,
    pub line2: String,
    pub epoch: DateTime<Utc>,
}

/// TLE batcher
pub struct TleBatcher {
    pending_tles: HashMap<SatelliteId, TleData>,
    batch_size: usize,
}

impl TleBatcher {
    pub fn new(batch_size: usize) -> Self {
        Self {
            pending_tles: HashMap::new(),
            batch_size,
        }
    }
    
    /// Add a TLE to the pending batch
    pub fn add_tle(&mut self, satellite_id: SatelliteId, tle: TleData) {
        self.pending_tles.insert(satellite_id, tle);
    }
    
    /// Check if batch is ready to flush
    pub fn is_ready(&self) -> bool {
        self.pending_tles.len() >= self.batch_size
    }
    
    /// Flush the current batch
    pub fn flush(&mut self) -> Option<TleBatch> {
        if self.pending_tles.is_empty() {
            return None;
        }
        
        let batch = TleBatch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            tles: self.pending_tles.clone(),
            snapshot_time: Utc::now(),
        };
        
        self.pending_tles.clear();
        Some(batch)
    }
    
    /// Force flush regardless of batch size
    pub fn force_flush(&mut self) -> Option<TleBatch> {
        self.flush()
    }
    
    /// Get pending count
    pub fn pending_count(&self) -> usize {
        self.pending_tles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tle_batching() {
        let mut batcher = TleBatcher::new(3);
        
        batcher.add_tle("SAT1".to_string(), TleData {
            line1: "1".to_string(),
            line2: "2".to_string(),
            epoch: Utc::now(),
        });
        
        assert!(!batcher.is_ready());
        
        batcher.add_tle("SAT2".to_string(), TleData {
            line1: "1".to_string(),
            line2: "2".to_string(),
            epoch: Utc::now(),
        });
        batcher.add_tle("SAT3".to_string(), TleData {
            line1: "1".to_string(),
            line2: "2".to_string(),
            epoch: Utc::now(),
        });
        
        assert!(batcher.is_ready());
        
        let batch = batcher.flush().unwrap();
        assert_eq!(batch.tles.len(), 3);
    }
}

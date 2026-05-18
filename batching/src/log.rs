//! Log entry batching for efficient storage

use bitemporal::log::LogEntry;
use serde::{Deserialize, Serialize};

/// Log batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogBatch {
    /// Batch ID
    pub batch_id: String,
    /// Log entries
    pub entries: Vec<LogEntry>,
    /// When this batch was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Log batcher
pub struct LogBatcher {
    pending_entries: Vec<LogEntry>,
    batch_size: usize,
}

impl LogBatcher {
    pub fn new(batch_size: usize) -> Self {
        Self {
            pending_entries: Vec::new(),
            batch_size,
        }
    }
    
    /// Add a log entry
    pub fn add_entry(&mut self, entry: LogEntry) {
        self.pending_entries.push(entry);
    }
    
    /// Check if batch is ready
    pub fn is_ready(&self) -> bool {
        self.pending_entries.len() >= self.batch_size
    }
    
    /// Flush the batch
    pub fn flush(&mut self) -> Option<LogBatch> {
        if self.pending_entries.is_empty() {
            return None;
        }
        
        let batch = LogBatch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            entries: self.pending_entries.clone(),
            created_at: chrono::Utc::now(),
        };
        
        self.pending_entries.clear();
        Some(batch)
    }
    
    /// Force flush
    pub fn force_flush(&mut self) -> Option<LogBatch> {
        self.flush()
    }
}

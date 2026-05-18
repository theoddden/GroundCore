//! Demodulator output batching for Float Protocols

use bitemporal::timestamp::{BiTemporal, EventTime, ReceptionTime};
use serde::{Deserialize, Serialize};

/// Demodulator batch for output to Float Protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemodulatorBatch {
    /// Batch ID
    pub batch_id: String,
    /// Decoded symbols
    pub symbols: Vec<BiTemporal<u8>>,
    /// Batch sequence number
    pub sequence: u64,
}

/// Demodulator batcher
pub struct DemodulatorBatcher {
    pending_symbols: Vec<BiTemporal<u8>>,
    batch_size: usize,
    sequence: u64,
}

impl DemodulatorBatcher {
    pub fn new(batch_size: usize) -> Self {
        Self {
            pending_symbols: Vec::new(),
            batch_size,
            sequence: 0,
        }
    }
    
    /// Add a decoded symbol
    pub fn add_symbol(&mut self, symbol: BiTemporal<u8>) {
        self.pending_symbols.push(symbol);
    }
    
    /// Check if batch is ready
    pub fn is_ready(&self) -> bool {
        self.pending_symbols.len() >= self.batch_size
    }
    
    /// Flush the current batch
    pub fn flush(&mut self) -> Option<DemodulatorBatch> {
        if self.pending_symbols.is_empty() {
            return None;
        }
        
        let batch = DemodulatorBatch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            symbols: self.pending_symbols.clone(),
            sequence: self.sequence,
        };
        
        self.pending_symbols.clear();
        self.sequence += 1;
        Some(batch)
    }
    
    /// Force flush
    pub fn force_flush(&mut self) -> Option<DemodulatorBatch> {
        self.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_demodulator_batching() {
        let mut batcher = DemodulatorBatcher::new(10);
        
        let symbol = BiTemporal::new(
            42,
            EventTime::new(Utc::now()),
            ReceptionTime::new(Utc::now()),
        );
        
        for _ in 0..10 {
            batcher.add_symbol(symbol.clone());
        }
        
        assert!(batcher.is_ready());
        
        let batch = batcher.flush().unwrap();
        assert_eq!(batch.symbols.len(), 10);
        assert_eq!(batch.sequence, 0);
    }
}

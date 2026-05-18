// Ethernet-over-OCT abstractions

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FramingError {
    #[error("Invalid frame length: {0}")]
    InvalidLength(usize),

    #[error("Frame checksum mismatch")]
    ChecksumMismatch,

    #[error("Frame deserialization error: {0}")]
    DeserializationError(String),
}

/// Ethernet frame for OCT
#[derive(Debug, Clone)]
pub struct EthernetFrame {
    pub payload: Vec<u8>,
    pub sequence_number: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl EthernetFrame {
    pub fn new(payload: Vec<u8>, sequence_number: u32) -> Self {
        Self {
            payload,
            sequence_number,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn len(&self) -> usize {
        self.payload.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

/// Frame metadata
#[derive(Debug, Clone)]
pub struct FrameMetadata {
    pub sequence_number: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub payload_length: usize,
    pub checksum: u32,
}

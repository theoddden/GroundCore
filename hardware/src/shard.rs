//! Pass-isolated shards for failure isolation
//!
//! Each active pass gets its own memory shard. Hardware allocations, ring buffers,
//! demodulator state, and bi-temporal log entries all live in the pass's shard.
//! No shared mutable state across passes.

use bumpalo::Bump;
use chrono::{DateTime, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pass-isolated memory shard
#[derive(Debug)]
pub struct PassShard {
    /// Pass identifier
    pub pass_id: PassId,
    /// Bump allocator for no-heap allocations within the shard
    arena: Bump,
    /// Ring buffers for sample storage
    ring_buffers: Vec<RingBuffer>,
    /// Pass state
    state: PassState,
    /// Shard-local bi-temporal log
    bitemporal_log: ShardLocalLog,
    /// When this shard was created
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
}

impl PassShard {
    /// Create a new pass shard with pre-allocated memory
    pub fn new(pass_id: PassId, arena_size: usize) -> Self {
        Self {
            pass_id,
            arena: Bump::with_capacity(arena_size),
            ring_buffers: Vec::new(),
            state: PassState::new(),
            bitemporal_log: ShardLocalLog::new(),
            created_at: Utc::now(),
        }
    }

    /// Allocate within the shard (no global allocator touched)
    pub fn allocate_in_shard<T>(&self, value: T) -> &T {
        self.arena.alloc(value)
    }

    /// Allocate a mutable value within the shard
    pub fn allocate_in_shard_mut<T>(&self, value: T) -> &mut T {
        self.arena.alloc(value)
    }

    /// Add a ring buffer to the shard
    pub fn add_ring_buffer(&mut self, buffer: RingBuffer) {
        self.ring_buffers.push(buffer);
    }

    /// Get pass state
    pub fn state(&self) -> &PassState {
        &self.state
    }

    /// Get mutable pass state
    pub fn state_mut(&mut self) -> &mut PassState {
        &mut self.state
    }

    /// Get shard-local log
    pub fn log(&self) -> &ShardLocalLog {
        &self.bitemporal_log
    }

    /// Get mutable shard-local log
    pub fn log_mut(&mut self) -> &mut ShardLocalLog {
        &mut self.bitemporal_log
    }

    /// Allocate a slice of `size` elements within the shard arena.
    ///
    /// # Safety
    /// The returned slice is valid only for the lifetime of this `PassShard`.
    /// It **must not** be used after `reset()` is called, as the arena memory
    /// will be reclaimed. `RingBuffer` instances referencing arena memory must
    /// be dropped (via `ring_buffers.clear()`) before `reset()` is invoked.
    pub fn allocate_slice_mut<T: Default + Copy>(&self, size: usize) -> &mut [T] {
        self.arena.alloc_slice_fill_default::<T>(size)
    }

    /// Reset the shard (called when pass completes)
    pub fn reset(&mut self) {
        // SAFETY: RingBuffers hold raw pointers into the arena and must be
        // dropped before the arena is reset to prevent dangling pointer UB.
        self.ring_buffers.clear();
        // Arena reset invalidates all previous allocations — ring_buffers
        // must already be empty by this point.
        self.arena.reset();
        self.state = PassState::new();
        self.bitemporal_log = ShardLocalLog::new();
    }
}

/// Pass state within a shard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassState {
    /// Whether the pass is currently active
    pub active: bool,
    /// Hardware allocated to this pass
    pub allocated_hardware: Vec<String>,
    /// Current demodulator state (serialized)
    pub demodulator_state: Option<Vec<u8>>,
    /// Sample count
    pub sample_count: u64,
    /// Byte count
    pub byte_count: u64,
}

impl PassState {
    pub fn new() -> Self {
        Self {
            active: false,
            allocated_hardware: Vec::new(),
            demodulator_state: None,
            sample_count: 0,
            byte_count: 0,
        }
    }
}

impl Default for PassState {
    fn default() -> Self {
        Self::new()
    }
}

/// Ring buffer for sample storage within a shard
#[derive(Debug)]
pub struct RingBuffer {
    /// Buffer pointer (allocated in shard arena)
    buffer: *mut f32,
    /// Capacity in samples
    capacity: usize,
    /// Current head index
    head: usize,
    /// Current tail index
    tail: usize,
}

impl RingBuffer {
    /// Create a new ring buffer (must be allocated within shard arena)
    ///
    /// # Safety
    ///
    /// The `buffer` pointer must be valid for at least `capacity` elements
    /// and must remain valid for the lifetime of the RingBuffer.
    pub unsafe fn new(buffer: *mut f32, capacity: usize) -> Self {
        Self {
            buffer,
            capacity,
            head: 0,
            tail: 0,
        }
    }

    /// Write samples to the ring buffer
    ///
    /// # Safety
    ///
    /// The `samples` slice must be valid and the ring buffer must have
    /// sufficient capacity to hold the data being written.
    pub unsafe fn write(&mut self, samples: &[f32]) -> Result<usize> {
        let available = self.available();
        let count = samples.len().min(available);

        for (i, &sample) in samples.iter().enumerate().take(count) {
            let idx = (self.head + i) % self.capacity;
            unsafe {
                *self.buffer.add(idx) = sample;
            }
        }

        self.head = (self.head + count) % self.capacity;
        Ok(count)
    }

    /// Read samples from the ring buffer
    ///
    /// # Safety
    ///
    /// The `buffer` slice must be valid and the ring buffer must have
    /// sufficient data to fill the buffer.
    pub unsafe fn read(&mut self, buffer: &mut [f32]) -> Result<usize> {
        let available = self.used();
        let count = buffer.len().min(available);

        for (i, item) in buffer.iter_mut().enumerate().take(count) {
            let idx = (self.tail + i) % self.capacity;
            unsafe {
                *item = *self.buffer.add(idx);
            }
        }

        self.tail = (self.tail + count) % self.capacity;
        Ok(count)
    }

    /// Available space for writing
    fn available(&self) -> usize {
        if self.head >= self.tail {
            self.capacity - (self.head - self.tail) - 1
        } else {
            self.tail - self.head - 1
        }
    }

    /// Used space (samples available for reading)
    fn used(&self) -> usize {
        if self.head >= self.tail {
            self.head - self.tail
        } else {
            self.capacity - (self.tail - self.head)
        }
    }
}

/// Shard-local bi-temporal log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardLocalLog {
    /// Log entries indexed by sample ID
    entries: HashMap<u64, LogEntry>,
}

impl ShardLocalLog {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Append a log entry
    pub fn append(&mut self, sample_id: u64, entry: LogEntry) {
        self.entries.insert(sample_id, entry);
    }

    /// Get a log entry
    pub fn get(&self, sample_id: u64) -> Option<&LogEntry> {
        self.entries.get(&sample_id)
    }

    /// Get all entries
    pub fn entries(&self) -> &HashMap<u64, LogEntry> {
        &self.entries
    }
}

impl Default for ShardLocalLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Log entry with bi-temporal timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Event time (when satellite emitted)
    pub event_time: DateTime<Utc>,
    /// Reception time (when ground station received)
    pub reception_time: DateTime<Utc>,
    /// Entry type
    pub entry_type: LogEntryType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntryType {
    /// Sample received
    SampleReceived,
    /// Demodulator state change
    DemodulatorStateChange,
    /// Hardware allocation
    HardwareAllocation,
    /// Error occurred
    Error(String),
}

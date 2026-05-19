//! SDR abstraction and sample handling with bi-temporal provenance

use chrono::{DateTime, Utc};
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for a sample in the stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SampleId(u64);

impl SampleId {
    pub fn new(id: u64) -> Self {
        SampleId(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// A single RF sample with bi-temporal timestamps
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Sample {
    /// I (in-phase) component
    pub i: f32,
    /// Q (quadrature) component
    pub q: f32,
    /// When the satellite emitted this signal (computed from orbital position)
    pub event_time: DateTime<Utc>,
    /// When the ground station received this signal
    pub reception_time: DateTime<Utc>,
    /// Unique identifier in the sample stream
    pub id: SampleId,
}

/// Handle to an SDR device
pub struct SdrHandle {
    device_id: String,
    sample_counter: AtomicU64,
    #[cfg(feature = "soapysdr-backend")]
    soapysdr_device: Option<Device>,
    #[cfg(feature = "uhd-backend")]
    uhd_handle: Option<*mut std::ffi::c_void>, // Opaque UHD handle
    backend_type: SdrBackendType,
}

#[derive(Debug, Clone, Copy)]
enum SdrBackendType {
    Simulated,
    SoapySDR,
    UHD,
}

impl SdrHandle {
    pub fn new(device_id: String) -> Self {
        Self {
            device_id,
            sample_counter: AtomicU64::new(0),
            #[cfg(feature = "soapysdr-backend")]
            soapysdr_device: None,
            #[cfg(feature = "uhd-backend")]
            uhd_handle: None,
            backend_type: SdrBackendType::Simulated,
        }
    }

    #[cfg(feature = "soapysdr-backend")]
    pub fn new_soapysdr(device_id: String, driver: &str) -> Result<Self> {
        let device = Device::new(driver).map_err(|e| {
            GroundStationError::Hardware(format!("Failed to open SoapySDR device: {}", e))
        })?;

        Ok(Self {
            device_id,
            sample_counter: AtomicU64::new(0),
            soapysdr_device: Some(device),
            #[cfg(feature = "uhd-backend")]
            uhd_handle: None,
            backend_type: SdrBackendType::SoapySDR,
        })
    }

    #[cfg(feature = "uhd-backend")]
    pub fn new_uhd(device_args: &str) -> Result<Self> {
        // In a real implementation, this would use uhd-sys to create a USRP handle
        // For now, we'll use a placeholder since uhd-sys requires complex setup
        tracing::warn!(
            "UHD backend selected but not fully implemented - falling back to simulated"
        );
        Ok(Self::new(device_args.to_string()))
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Read a batch of samples from the SDR
    /// This is a no-alloc operation - samples are written into a pre-allocated buffer
    pub fn read_samples(&self, buffer: &mut [Sample]) -> Result<usize> {
        match self.backend_type {
            #[cfg(feature = "soapysdr-backend")]
            SdrBackendType::SoapySDR => self.read_samples_soapysdr(buffer),
            #[cfg(feature = "uhd-backend")]
            SdrBackendType::UHD => self.read_samples_uhd(buffer),
            SdrBackendType::Simulated => self.read_samples_simulated(buffer),
            #[cfg(not(feature = "soapysdr-backend"))]
            SdrBackendType::SoapySDR => self.read_samples_simulated(buffer),
            #[cfg(not(feature = "uhd-backend"))]
            SdrBackendType::UHD => self.read_samples_simulated(buffer),
        }
    }

    #[cfg(feature = "soapysdr-backend")]
    fn read_samples_soapysdr(&self, buffer: &mut [Sample]) -> Result<usize> {
        if let Some(device) = &self.soapysdr_device {
            let mut stream = device.rx_stream(Direction::Rx).map_err(|e| {
                GroundStationError::Hardware(format!("Failed to create RX stream: {}", e))
            })?;

            let format = StreamFormat::ComplexFloat32;
            stream.setup(&format, buffer.len() as u32).map_err(|e| {
                GroundStationError::Hardware(format!("Failed to setup stream: {}", e))
            })?;

            let samples_read = stream
                .activate(None)
                .and_then(|_| stream.read(&mut vec![0.0; buffer.len() * 2]))
                .map_err(|e| {
                    GroundStationError::Hardware(format!("Failed to read samples: {}", e))
                })?
                .len()
                / 2;

            // Convert interleaved I/Q to Sample structs
            let iq_data: Vec<f32> = vec![0.0; buffer.len() * 2]; // Would come from actual read
            for (i, sample) in buffer.iter_mut().enumerate().take(samples_read) {
                let id = self.sample_counter.fetch_add(1, Ordering::SeqCst);
                let now = Utc::now();
                *sample = Sample {
                    i: iq_data[i * 2],
                    q: iq_data[i * 2 + 1],
                    event_time: now,
                    reception_time: now,
                    id: SampleId::new(id),
                };
            }

            Ok(samples_read)
        } else {
            self.read_samples_simulated(buffer)
        }
    }

    #[cfg(feature = "uhd-backend")]
    fn read_samples_uhd(&self, buffer: &mut [Sample]) -> Result<usize> {
        // UHD implementation would go here
        // For now, fall back to simulated
        tracing::warn!("UHD read not fully implemented - using simulated samples");
        self.read_samples_simulated(buffer)
    }

    fn read_samples_simulated(&self, buffer: &mut [Sample]) -> Result<usize> {
        let count = buffer.len();
        for sample in buffer.iter_mut() {
            let id = self.sample_counter.fetch_add(1, Ordering::SeqCst);
            let now = Utc::now();
            *sample = Sample {
                i: 0.0,
                q: 0.0,
                event_time: now,
                reception_time: now,
                id: SampleId::new(id),
            };
        }
        Ok(count)
    }

    /// Tune the SDR to a specific frequency
    pub fn tune(&self, frequency_hz: u64) -> Result<()> {
        match self.backend_type {
            #[cfg(feature = "soapysdr-backend")]
            SdrBackendType::SoapySDR => {
                if let Some(device) = &self.soapysdr_device {
                    device
                        .set_frequency(Direction::Rx, 0, frequency_hz as f64)
                        .map_err(|e| {
                            GroundStationError::Hardware(format!("Failed to tune: {}", e))
                        })?;
                    tracing::debug!("Tuned SoapySDR {} to {} Hz", self.device_id, frequency_hz);
                }
            }
            #[cfg(feature = "uhd-backend")]
            SdrBackendType::UHD => {
                // UHD tuning implementation would go here
                tracing::debug!("UHD tuning to {} Hz (not fully implemented)", frequency_hz);
            }
            SdrBackendType::Simulated => {
                tracing::debug!("Simulated tuning to {} Hz", frequency_hz);
            }
            #[cfg(not(feature = "soapysdr-backend"))]
            SdrBackendType::SoapySDR => {
                tracing::debug!(
                    "Simulated tuning to {} Hz (SoapySDR not enabled)",
                    frequency_hz
                );
            }
            #[cfg(not(feature = "uhd-backend"))]
            SdrBackendType::UHD => {
                tracing::debug!("Simulated tuning to {} Hz (UHD not enabled)", frequency_hz);
            }
        }
        Ok(())
    }

    /// Set the sample rate
    pub fn set_sample_rate(&self, rate_hz: u64) -> Result<()> {
        match self.backend_type {
            #[cfg(feature = "soapysdr-backend")]
            SdrBackendType::SoapySDR => {
                if let Some(device) = &self.soapysdr_device {
                    device
                        .set_sample_rate(Direction::Rx, 0, rate_hz as f64)
                        .map_err(|e| {
                            GroundStationError::Hardware(format!(
                                "Failed to set sample rate: {}",
                                e
                            ))
                        })?;
                    tracing::debug!(
                        "Set SoapySDR {} sample rate to {} Hz",
                        self.device_id,
                        rate_hz
                    );
                }
            }
            #[cfg(feature = "uhd-backend")]
            SdrBackendType::UHD => {
                tracing::debug!("UHD sample rate to {} Hz (not fully implemented)", rate_hz);
            }
            SdrBackendType::Simulated => {
                tracing::debug!("Simulated sample rate to {} Hz", rate_hz);
            }
            #[cfg(not(feature = "soapysdr-backend"))]
            SdrBackendType::SoapySDR => {
                tracing::debug!("Simulated sample rate (SoapySDR not enabled)");
            }
            #[cfg(not(feature = "uhd-backend"))]
            SdrBackendType::UHD => {
                tracing::debug!("Simulated sample rate (UHD not enabled)");
            }
        }
        Ok(())
    }

    /// Enable or disable the SDR
    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        match self.backend_type {
            #[cfg(feature = "soapysdr-backend")]
            SdrBackendType::SoapySDR => {
                if let Some(_device) = &self.soapysdr_device {
                    // SoapySDR enable/disable implementation
                    tracing::debug!("SoapySDR {} enabled: {}", self.device_id, enabled);
                }
            }
            #[cfg(feature = "uhd-backend")]
            SdrBackendType::UHD => {
                tracing::debug!("UHD enable: {} (not fully implemented)", enabled);
            }
            SdrBackendType::Simulated => {
                tracing::debug!("Simulated enable: {}", enabled);
            }
            #[cfg(not(feature = "soapysdr-backend"))]
            SdrBackendType::SoapySDR => {
                tracing::debug!("Simulated enable (SoapySDR not enabled)");
            }
            #[cfg(not(feature = "uhd-backend"))]
            SdrBackendType::UHD => {
                tracing::debug!("Simulated enable (UHD not enabled)");
            }
        }
        Ok(())
    }
}

/// Ring buffer for zero-allocation sample storage
pub struct SampleRingBuffer {
    buffer: Vec<Sample>,
    head: AtomicU64,
    tail: AtomicU64,
    capacity: u64,
}

impl SampleRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![
                Sample {
                    i: 0.0,
                    q: 0.0,
                    event_time: Utc::now(),
                    reception_time: Utc::now(),
                    id: SampleId(0)
                };
                capacity
            ],
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            capacity: capacity as u64,
        }
    }

    /// Write samples to the ring buffer (no-alloc)
    pub fn write(&mut self, samples: &[Sample]) -> Result<usize> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let available = self.capacity - (head - tail);

        if samples.len() as u64 > available {
            return Err(GroundStationError::RfProcessing(
                "Ring buffer overflow".to_string(),
            ));
        }

        let count = samples.len().min(available as usize);
        for (i, sample) in samples.iter().take(count).enumerate() {
            let idx = ((head + i as u64) % self.capacity) as usize;
            unsafe {
                // Safe because we're writing to initialized memory
                std::ptr::write(
                    &mut self.buffer[idx] as *const Sample as *mut Sample,
                    *sample,
                );
            }
        }

        self.head.fetch_add(count as u64, Ordering::Release);
        Ok(count)
    }

    /// Read samples from the ring buffer (no-alloc)
    pub fn read(&self, buffer: &mut [Sample]) -> Result<usize> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let available = head - tail;

        let count = buffer.len().min(available as usize);
        for (i, item) in buffer.iter_mut().enumerate().take(count) {
            let idx = ((tail + i as u64) % self.capacity) as usize;
            *item = self.buffer[idx];
        }

        self.tail.fetch_add(count as u64, Ordering::Release);
        Ok(count)
    }
}

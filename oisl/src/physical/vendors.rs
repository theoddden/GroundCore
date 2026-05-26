// Vendor implementations
//
// Mynaric CONDOR Mk3, Tesat SCOT80, and other terminal implementations
// of the OpticalTerminal trait.

use crate::physical::OpticalTerminal;
use crate::physical::terminal::{
    AcquisitionSchedule, CalibrationReport, EthernetEndpoint, HealthReport, PatHandle,
    TelemetryFrame, TelemetryStream, TerminalCapability, TerminalError, TerminalStatus,
};
use crate::topology::link_state::LinkPhase;
use crate::{DataRate, OctConfiguration, OctStandardVersion, ResetLevel, TerminalId};
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::mpsc as tokio_mpsc;

/// Vendor enum
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Vendor {
    Mynaric,
    Tesat,
    Skyloom,
    Caci,
    Other(String),
}

/// Serial number
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerialNumber(pub String);

/// Mynaric CONDOR Mk3 implementation
pub struct CondorMk3 {
    _terminal_id: TerminalId,
    serial: SerialNumber,
    api_endpoint: String,
    config: Option<OctConfiguration>,
}

impl CondorMk3 {
    pub fn new(terminal_id: TerminalId, serial: SerialNumber, api_endpoint: String) -> Self {
        Self {
            _terminal_id: terminal_id,
            serial,
            api_endpoint,
            config: None,
        }
    }

    async fn send_command(&self, command: &str) -> Result<String, TerminalError> {
        // Mynaric uses REST API for management
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/v1/command", self.api_endpoint))
            .json(&serde_json::json!({ "command": command }))
            .send()
            .await
            .map_err(|e| TerminalError::CommunicationError(e.to_string()))?;

        let text = response
            .text()
            .await
            .map_err(|e| TerminalError::CommunicationError(e.to_string()))?;

        Ok(text)
    }
}

#[async_trait]
impl OpticalTerminal for CondorMk3 {
    fn vendor(&self) -> Vendor {
        Vendor::Mynaric
    }

    fn model(&self) -> &str {
        "CONDOR Mk3"
    }

    fn serial(&self) -> SerialNumber {
        self.serial.clone()
    }

    fn oct_standard_versions(&self) -> Vec<OctStandardVersion> {
        vec![OctStandardVersion::V4_0_0]
    }

    fn capabilities(&self) -> TerminalCapability {
        TerminalCapability {
            max_data_rate: DataRate(10_000_000_000), // 10 Gbps
            max_pointing_accuracy_rad: 10e-6,        // 10 microrad
            supported_standards: vec![OctStandardVersion::V4_0_0],
            beam_divergence_mrad: 0.1,
            max_range_km: 5000.0,
        }
    }

    async fn configure(&mut self, config: OctConfiguration) -> Result<(), TerminalError> {
        let config_json = serde_json::to_string(&config)
            .map_err(|e| TerminalError::ConfigurationError(e.to_string()))?;

        self.send_command(&format!("configure {}", config_json))
            .await?;
        self.config = Some(config);
        Ok(())
    }

    async fn calibrate(&mut self) -> Result<CalibrationReport, TerminalError> {
        let _response = self.send_command("calibrate").await?;

        // Parse response (simplified)
        Ok(CalibrationReport {
            calibrated_at: Utc::now(),
            pointing_accuracy_rad: 5e-6,
            alignment_error_rad: 2e-6,
            next_calibration_due: Utc::now() + chrono::Duration::days(30),
        })
    }

    async fn schedule_acquisition(
        &mut self,
        schedule: AcquisitionSchedule,
    ) -> Result<(), TerminalError> {
        let schedule_json =
            serde_json::to_string(&schedule).map_err(|e| TerminalError::PatError(e.to_string()))?;

        self.send_command(&format!("schedule_acquisition {}", schedule_json))
            .await?;
        Ok(())
    }

    async fn begin_pat(&mut self) -> Result<PatHandle, TerminalError> {
        let _response = self.send_command("begin_pat").await?;

        Ok(PatHandle {
            acquisition_id: crate::AcquisitionId::new_v4(),
            started_at: Utc::now(),
        })
    }

    async fn cancel_pat(
        &mut self,
        acquisition_id: crate::AcquisitionId,
    ) -> Result<(), TerminalError> {
        self.send_command(&format!("cancel_pat {}", acquisition_id))
            .await?;
        Ok(())
    }

    async fn data_path(&self) -> Result<EthernetEndpoint, TerminalError> {
        Ok(EthernetEndpoint {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            vlan: Some(100),
        })
    }

    async fn telemetry_stream(&self) -> Result<TelemetryStream, TerminalError> {
        let (tx, rx) = tokio_mpsc::channel(100);

        // Simulate telemetry stream
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let frame = TelemetryFrame {
                    timestamp: Utc::now(),
                    data: vec![0u8; 1024],
                };
                if tx.send(frame).await.is_err() {
                    break;
                }
            }
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    async fn health_check(&self) -> Result<HealthReport, TerminalError> {
        let _response = self.send_command("health_check").await?;

        Ok(HealthReport {
            overall_health: crate::HealthMetrics(crate::ConfidenceScore::new(0.95)),
            component_health: vec![
                ("laser".to_string(), 0.98),
                ("pointing".to_string(), 0.97),
                ("detector".to_string(), 0.96),
            ],
            warnings: vec![],
            errors: vec![],
            reported_at: Utc::now(),
        })
    }

    async fn reset(&mut self, level: ResetLevel) -> Result<(), TerminalError> {
        self.send_command(&format!("reset {:?}", level)).await?;
        Ok(())
    }

    async fn safe_mode(&mut self) -> Result<(), TerminalError> {
        self.send_command("safe_mode").await?;
        Ok(())
    }

    async fn get_status(&self) -> Result<TerminalStatus, TerminalError> {
        let _response = self.send_command("status").await?;

        Ok(TerminalStatus {
            operational: true,
            current_phase: LinkPhase::Idle,
            current_link: None,
            temperature_c: 25.0,
            power_watts: 150.0,
            last_updated: Utc::now(),
        })
    }
}

/// Tesat SCOT80 implementation
pub struct Scot80 {
    _terminal_id: TerminalId,
    serial: SerialNumber,
    _control_interface: String,
    config: Option<OctConfiguration>,
}

impl Scot80 {
    pub fn new(terminal_id: TerminalId, serial: SerialNumber, control_interface: String) -> Self {
        Self {
            _terminal_id: terminal_id,
            serial,
            _control_interface: control_interface,
            config: None,
        }
    }

    async fn send_command(&self, _command: &[u8]) -> Result<Vec<u8>, TerminalError> {
        // Tesat uses proprietary protocol (simplified)
        // In production, this would use the actual Tesat control protocol
        Ok(vec![])
    }
}

#[async_trait]
impl OpticalTerminal for Scot80 {
    fn vendor(&self) -> Vendor {
        Vendor::Tesat
    }

    fn model(&self) -> &str {
        "SCOT80"
    }

    fn serial(&self) -> SerialNumber {
        self.serial.clone()
    }

    fn oct_standard_versions(&self) -> Vec<OctStandardVersion> {
        vec![OctStandardVersion::V3_1_0, OctStandardVersion::V3_2_0]
    }

    fn capabilities(&self) -> TerminalCapability {
        TerminalCapability {
            max_data_rate: DataRate(2_500_000_000), // 2.5 Gbps
            max_pointing_accuracy_rad: 15e-6,       // 15 microrad
            supported_standards: vec![OctStandardVersion::V3_1_0, OctStandardVersion::V3_2_0],
            beam_divergence_mrad: 0.15,
            max_range_km: 3000.0,
        }
    }

    async fn configure(&mut self, config: OctConfiguration) -> Result<(), TerminalError> {
        let config_bytes = serde_json::to_vec(&config)
            .map_err(|e| TerminalError::ConfigurationError(e.to_string()))?;

        self.send_command(&config_bytes).await?;
        self.config = Some(config);
        Ok(())
    }

    async fn calibrate(&mut self) -> Result<CalibrationReport, TerminalError> {
        self.send_command(b"CALIBRATE").await?;

        Ok(CalibrationReport {
            calibrated_at: Utc::now(),
            pointing_accuracy_rad: 8e-6,
            alignment_error_rad: 3e-6,
            next_calibration_due: Utc::now() + chrono::Duration::days(60),
        })
    }

    async fn schedule_acquisition(
        &mut self,
        schedule: AcquisitionSchedule,
    ) -> Result<(), TerminalError> {
        let schedule_bytes =
            serde_json::to_vec(&schedule).map_err(|e| TerminalError::PatError(e.to_string()))?;

        self.send_command(&schedule_bytes).await?;
        Ok(())
    }

    async fn begin_pat(&mut self) -> Result<PatHandle, TerminalError> {
        self.send_command(b"BEGIN_PAT").await?;

        Ok(PatHandle {
            acquisition_id: crate::AcquisitionId::new_v4(),
            started_at: Utc::now(),
        })
    }

    async fn cancel_pat(
        &mut self,
        acquisition_id: crate::AcquisitionId,
    ) -> Result<(), TerminalError> {
        let cmd = format!("CANCEL_PAT:{}", acquisition_id);
        self.send_command(cmd.as_bytes()).await?;
        Ok(())
    }

    async fn data_path(&self) -> Result<EthernetEndpoint, TerminalError> {
        Ok(EthernetEndpoint {
            ip: "10.0.1.1".to_string(),
            port: 8000,
            vlan: Some(200),
        })
    }

    async fn telemetry_stream(&self) -> Result<TelemetryStream, TerminalError> {
        let (tx, rx) = tokio_mpsc::channel(100);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let frame = TelemetryFrame {
                    timestamp: Utc::now(),
                    data: vec![0u8; 512],
                };
                if tx.send(frame).await.is_err() {
                    break;
                }
            }
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    async fn health_check(&self) -> Result<HealthReport, TerminalError> {
        self.send_command(b"HEALTH_CHECK").await?;

        Ok(HealthReport {
            overall_health: crate::HealthMetrics(crate::ConfidenceScore::new(0.92)),
            component_health: vec![
                ("laser".to_string(), 0.95),
                ("pointing".to_string(), 0.94),
                ("detector".to_string(), 0.93),
            ],
            warnings: vec![],
            errors: vec![],
            reported_at: Utc::now(),
        })
    }

    async fn reset(&mut self, level: ResetLevel) -> Result<(), TerminalError> {
        let cmd = format!("RESET:{:?}", level);
        self.send_command(cmd.as_bytes()).await?;
        Ok(())
    }

    async fn safe_mode(&mut self) -> Result<(), TerminalError> {
        self.send_command(b"SAFE_MODE").await?;
        Ok(())
    }

    async fn get_status(&self) -> Result<TerminalStatus, TerminalError> {
        self.send_command(b"STATUS").await?;

        Ok(TerminalStatus {
            operational: true,
            current_phase: LinkPhase::Idle,
            current_link: None,
            temperature_c: 22.0,
            power_watts: 120.0,
            last_updated: Utc::now(),
        })
    }
}

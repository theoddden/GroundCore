//! Antenna controller abstraction

use chrono::{DateTime, Utc};
use ground_core::{GroundStationError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pointing target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointingTarget {
    /// Azimuth in degrees (0-360)
    pub azimuth: f64,
    /// Elevation in degrees (0-90)
    pub elevation: f64,
    /// Target timestamp
    pub timestamp: DateTime<Utc>,
}

/// Controller status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerStatus {
    Idle,
    Slewing,
    Tracking,
    Homing,
    Stowed,
    Fault,
    EmergencyStopped,
}

/// Controller error
#[derive(Debug, thiserror::Error)]
pub enum ControllerError {
    #[error("Communication error: {0}")]
    Communication(String),
    #[error("Motor fault: {0}")]
    MotorFault(String),
    #[error("Position out of limits")]
    PositionOutOfLimits,
    #[error("Slew rate exceeded")]
    SlewRateExceeded,
    #[error("Emergency stop activated")]
    EmergencyStop,
    #[error("Hardware error: {0}")]
    Hardware(String),
}

/// Antenna controller trait
#[async_trait::async_trait]
pub trait AntennaController: Send + Sync {
    /// Point antenna at specific azimuth/elevation
    async fn point(&mut self, azimuth: f64, elevation: f64) -> Result<()>;

    /// Get current antenna position
    async fn get_position(&self) -> Result<(f64, f64)>; // (azimuth, elevation)

    /// Slew to target with motion planning
    async fn slew_to(&mut self, target: PointingTarget) -> Result<crate::slew::SlewId>;

    /// Abort current slew
    async fn abort_slew(&mut self) -> Result<()>;

    /// Stow antenna (safe parking position)
    async fn stow(&mut self) -> Result<()>;

    /// Get controller status
    async fn get_status(&self) -> Result<ControllerStatus>;

    /// Enable controller
    async fn enable(&mut self) -> Result<()>;

    /// Disable controller
    async fn disable(&mut self) -> Result<()>;

    /// Emergency stop
    async fn emergency_stop(&mut self) -> Result<()>;

    /// Home antenna (find index marks)
    async fn home(&mut self) -> Result<()>;
}

/// Generic antenna controller implementation
pub struct GenericAntennaController {
    #[allow(dead_code)]
    id: Uuid,
    status: ControllerStatus,
    current_azimuth: f64,
    current_elevation: f64,
    target_azimuth: f64,
    target_elevation: f64,
    azimuth_min: f64,
    azimuth_max: f64,
    elevation_min: f64,
    elevation_max: f64,
    #[allow(dead_code)]
    azimuth_rate_limit: f64, // degrees per second
    #[allow(dead_code)]
    elevation_rate_limit: f64, // degrees per second
    /// Stow azimuth (degrees). Defaults to 0° (north).
    stow_azimuth: f64,
    /// Stow elevation (degrees). Defaults to 5° to minimise wind loading on
    /// horizon-parked mounts. Use 90° only for specific dish designs that stow
    /// at zenith.
    stow_elevation: f64,
    enabled: bool,
}

impl GenericAntennaController {
    /// Create a new generic antenna controller.
    ///
    /// Stow position defaults to azimuth=0°, elevation=5° (horizon park). Pass
    /// explicit values via [`with_stow_position`] for mounts that park at zenith.
    pub fn new(
        azimuth_min: f64,
        azimuth_max: f64,
        elevation_min: f64,
        elevation_max: f64,
        azimuth_rate_limit: f64,
        elevation_rate_limit: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            status: ControllerStatus::Idle,
            current_azimuth: 0.0,
            current_elevation: 0.0,
            target_azimuth: 0.0,
            target_elevation: 0.0,
            azimuth_min,
            azimuth_max,
            elevation_min,
            elevation_max,
            azimuth_rate_limit,
            elevation_rate_limit,
            stow_azimuth: 0.0,
            stow_elevation: 5.0,
            enabled: false,
        }
    }

    /// Override the stow position. Use for dish mounts that park at zenith
    /// (`azimuth=0.0, elevation=90.0`) or at a custom safe-zone position.
    pub fn with_stow_position(mut self, azimuth: f64, elevation: f64) -> Self {
        self.stow_azimuth = azimuth;
        self.stow_elevation = elevation;
        self
    }
}

#[async_trait::async_trait]
impl AntennaController for GenericAntennaController {
    async fn point(&mut self, azimuth: f64, elevation: f64) -> Result<()> {
        if !self.enabled {
            return Err(GroundStationError::Hardware(
                "Controller disabled".to_string(),
            ));
        }

        // Validate position limits
        if azimuth < self.azimuth_min || azimuth > self.azimuth_max {
            return Err(GroundStationError::Validation(format!(
                "Azimuth {} out of range [{}, {}]",
                azimuth, self.azimuth_min, self.azimuth_max
            )));
        }

        if elevation < self.elevation_min || elevation > self.elevation_max {
            return Err(GroundStationError::Validation(format!(
                "Elevation {} out of range [{}, {}]",
                elevation, self.elevation_min, self.elevation_max
            )));
        }

        self.target_azimuth = azimuth;
        self.target_elevation = elevation;
        self.status = ControllerStatus::Slewing;

        // Simulate pointing (in real implementation, this would send command to hardware)
        self.current_azimuth = azimuth;
        self.current_elevation = elevation;
        self.status = ControllerStatus::Idle;

        Ok(())
    }

    async fn get_position(&self) -> Result<(f64, f64)> {
        Ok((self.current_azimuth, self.current_elevation))
    }

    async fn slew_to(&mut self, target: PointingTarget) -> Result<crate::slew::SlewId> {
        self.point(target.azimuth, target.elevation).await?;
        Ok(Uuid::new_v4())
    }

    async fn abort_slew(&mut self) -> Result<()> {
        self.status = ControllerStatus::Idle;
        Ok(())
    }

    async fn stow(&mut self) -> Result<()> {
        self.point(self.stow_azimuth, self.stow_elevation).await?;
        self.status = ControllerStatus::Stowed;
        Ok(())
    }

    async fn get_status(&self) -> Result<ControllerStatus> {
        Ok(self.status)
    }

    async fn enable(&mut self) -> Result<()> {
        self.enabled = true;
        Ok(())
    }

    async fn disable(&mut self) -> Result<()> {
        self.enabled = false;
        self.status = ControllerStatus::Idle;
        Ok(())
    }

    async fn emergency_stop(&mut self) -> Result<()> {
        self.status = ControllerStatus::EmergencyStopped;
        Ok(())
    }

    async fn home(&mut self) -> Result<()> {
        self.status = ControllerStatus::Homing;
        // Simulate homing procedure
        self.current_azimuth = 0.0;
        self.current_elevation = 0.0;
        self.status = ControllerStatus::Idle;
        Ok(())
    }
}

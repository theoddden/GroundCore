//! Digital twin runtime - the reconciliation loop
//!
//! This module implements the Tokio background task that runs alongside the pass shard.
//! Each 1-second tick: propagate forward, compute expected intervals, ingest observed
//! metrics, run divergence against the intervals, fire anomaly signal, snapshot twin state.

use crate::components::*;
use crate::divergence::{DivergenceDetector, MetricDivergence, MetricType};
use crate::snapshot::{SnapshotManager, TwinSnapshot, WorldSummary};
use crate::systems;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bitemporal::{EventTime, ReceptionTime};
use chrono::{DateTime, Duration, Utc};
use ground_core::{LinkId, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

/// Schedule label for the digital twin tick
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TwinTickSchedule;

/// Observed metrics from the hardware layer
#[derive(Debug, Clone)]
pub struct ObservedMetric {
    /// Link identifier
    pub link_id: LinkId,
    /// Metric type
    pub metric_type: MetricType,
    /// Observed value
    pub value: f64,
    /// Observation time
    pub observation_time: DateTime<Utc>,
}

/// Anomaly signal sent when divergence is detected
#[derive(Debug, Clone)]
pub struct AnomalySignal {
    /// The divergence record
    pub divergence: MetricDivergence,
    /// When the signal was generated
    pub signal_time: DateTime<Utc>,
}

/// Digital twin runtime configuration
#[derive(Debug, Clone)]
pub struct TwinConfig {
    /// Tick interval (default: 1 second)
    pub tick_interval: Duration,
    /// Forecast horizon (default: 5 minutes)
    pub forecast_horizon: Duration,
    /// Whether the runtime is running
    pub running: bool,
}

impl Default for TwinConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::seconds(1),
            forecast_horizon: Duration::minutes(5),
            running: false,
        }
    }
}

/// Digital twin runtime
///
/// This is the main reconciliation loop that runs as a Tokio background task.
pub struct DigitalTwinRuntime {
    /// Bevy ECS world
    world: World,
    /// Divergence detector
    divergence_detector: DivergenceDetector,
    /// Configuration
    config: TwinConfig,
    /// Channel for receiving observed metrics
    observed_rx: mpsc::Receiver<ObservedMetric>,
    /// Channel for sending anomaly signals
    anomaly_tx: mpsc::Sender<AnomalySignal>,
    /// Channel for sending twin forecasts
    forecast_tx: watch::Sender<TwinForecast>,
    /// External snapshotting backend (schedule rollback, audit log)
    #[allow(dead_code)]
    snapshot_manager: Arc<snapshotting::SnapshotManager>,
    /// Local twin snapshot store for bi-temporal forensic replay
    twin_snapshot_store: SnapshotManager,
}

impl DigitalTwinRuntime {
    /// Create a new digital twin runtime
    pub fn new(
        observed_rx: mpsc::Receiver<ObservedMetric>,
        anomaly_tx: mpsc::Sender<AnomalySignal>,
        forecast_tx: watch::Sender<TwinForecast>,
        snapshot_manager: Arc<snapshotting::SnapshotManager>,
    ) -> Self {
        let mut world = World::new();

        // Add the tick schedule
        let mut schedule = Schedule::default();
        schedule.add_systems((
            systems::propagate_orbits,
            systems::compute_link_intervals,
            systems::visibility_windows,
        ));
        world.add_schedule(schedule);

        Self {
            world,
            divergence_detector: DivergenceDetector::default(),
            config: TwinConfig::default(),
            observed_rx,
            anomaly_tx,
            forecast_tx,
            snapshot_manager,
            twin_snapshot_store: SnapshotManager::default(),
        }
    }

    /// Add a satellite entity to the twin
    pub fn add_satellite(
        &mut self,
        satellite_id: ground_core::SatelliteId,
        norad_id: u32,
        name: String,
    ) -> Entity {
        self.world
            .spawn((
                SatelliteIdComponent::new(satellite_id, norad_id, name),
                Position::new(
                    nalgebra::Vector3::zeros(),
                    nalgebra::Vector3::zeros(),
                    Utc::now(),
                ),
            ))
            .id()
    }

    /// Add a ground station entity to the twin
    pub fn add_ground_station(
        &mut self,
        station_id: ground_core::StationId,
        lat: f64,
        lon: f64,
        alt: f64,
    ) -> Entity {
        self.world
            .spawn((
                GroundStation::new(station_id, lat, lon, alt),
                Position::new(
                    nalgebra::Vector3::zeros(),
                    nalgebra::Vector3::zeros(),
                    Utc::now(),
                ),
            ))
            .id()
    }

    /// Add a link entity to the twin
    pub fn add_link(&mut self, link_id: LinkId) -> Entity {
        self.world
            .spawn((
                LinkBudget::new(link_id),
                OpticalTerminalState::new("Unknown".to_string(), "terminal-1".to_string()),
            ))
            .id()
    }

    /// Run the twin runtime
    pub async fn run(&mut self) -> Result<()> {
        self.config.running = true;
        info!("Digital twin runtime started");

        let mut tick_interval = tokio::time::interval(self.config.tick_interval.to_std().unwrap());
        tick_interval.tick().await; // Skip first tick immediately

        while self.config.running {
            tick_interval.tick().await;

            // Run the tick
            self.tick().await?;

            // Check for observed metrics
            while let Ok(observed) = self.observed_rx.try_recv() {
                self.process_observed(observed).await?;
            }
        }

        info!("Digital twin runtime stopped");
        Ok(())
    }

    /// Stop the twin runtime
    pub fn stop(&mut self) {
        self.config.running = false;
    }

    /// Execute a single tick of the twin
    async fn tick(&mut self) -> Result<()> {
        debug!("Digital twin tick at {}", Utc::now());

        // Run the tick schedule (physics systems)
        self.world.run_schedule(TwinTickSchedule);

        // Generate forecast
        let forecast = self.generate_forecast();
        if let Err(e) = self.forecast_tx.send(forecast) {
            error!("Failed to send twin forecast: {}", e);
        }

        // Snapshot twin state
        self.snapshot_state().await?;

        Ok(())
    }

    /// Process an observed metric
    async fn process_observed(&mut self, observed: ObservedMetric) -> Result<()> {
        // Get the predicted interval for this link/metric from the ECS world
        let predicted_interval =
            self.get_predicted_interval(&observed.link_id, observed.metric_type)?;

        // Process through divergence detector
        let prediction_time = Utc::now(); // In practice, this would be when the prediction was made
        let event_time = EventTime::new(prediction_time);
        let reception_time = ReceptionTime::new(observed.observation_time);

        if let Some(divergence) = self.divergence_detector.process_observation(
            observed.link_id,
            observed.metric_type,
            predicted_interval,
            observed.value,
            event_time,
            reception_time,
        ) {
            // Send anomaly signal
            let signal = AnomalySignal {
                divergence: divergence.clone(),
                signal_time: Utc::now(),
            };

            if let Err(e) = self.anomaly_tx.send(signal).await {
                error!("Failed to send anomaly signal: {}", e);
            }

            warn!(
                "Anomaly detected for link {:?} metric {:?}: observed {:.2} outside interval [{:.2}, {:.2}]",
                observed.link_id,
                observed.metric_type,
                observed.value,
                predicted_interval.lower,
                predicted_interval.upper
            );
        }

        Ok(())
    }

    /// Get the predicted interval for a link/metric from the ECS world
    fn get_predicted_interval(
        &mut self,
        link_id: &LinkId,
        metric_type: MetricType,
    ) -> Result<crate::divergence::Interval> {
        // Query the ECS world for the link budget
        let mut query = self.world.query::<&LinkBudget>();
        let interval = query
            .iter(&self.world)
            .find(|link_budget| link_budget.link_id == *link_id)
            .map(|link_budget| match metric_type {
                MetricType::Snr => link_budget.predicted_snr,
                MetricType::Ber => crate::divergence::Interval::new(0.0, 0.0), // Placeholder
                _ => crate::divergence::Interval::new(0.0, 0.0),               // Placeholder
            })
            .ok_or_else(|| {
                ground_core::GroundStationError::Twin(format!(
                    "Link {:?} not found in twin world",
                    link_id
                ))
            })?;

        Ok(interval)
    }

    /// Generate a twin forecast
    fn generate_forecast(&self) -> TwinForecast {
        let forecast_time = Utc::now() + self.config.forecast_horizon;

        // In a full implementation, this would:
        // 1. Propagate all entities forward to forecast_horizon
        // 2. Compute predicted intervals for all links
        // 3. Package into a TwinForecast struct

        TwinForecast {
            forecast_time,
            link_predictions: Vec::new(), // Placeholder
        }
    }

    /// Snapshot the twin state for bi-temporal forensic replay.
    ///
    /// Extracts observable quantities from the ECS world into a serializable
    /// `WorldSummary`, then stores the snapshot in the local twin snapshot store.
    async fn snapshot_state(&mut self) -> Result<()> {
        let mut summary = WorldSummary {
            entity_count: self.world.entities().len() as usize,
            ..WorldSummary::default()
        };

        // Harvest predicted SNR mid-points from ECS link budget components
        let mut query = self.world.query::<&LinkBudget>();
        for link_budget in query.iter(&self.world) {
            let snr_mid = (link_budget.predicted_snr.lower + link_budget.predicted_snr.upper) / 2.0;
            summary.link_snr_mid_db.insert(link_budget.link_id, snr_mid);
        }

        let snapshot = TwinSnapshot::new(
            Utc::now(),
            summary,
            self.divergence_detector.divergence_log().to_vec(),
        );

        self.twin_snapshot_store.store_snapshot(snapshot);
        Ok(())
    }
}

/// Twin forecast containing predicted states
#[derive(Debug, Clone)]
pub struct TwinForecast {
    /// When this forecast is valid
    pub forecast_time: DateTime<Utc>,
    /// Predicted states for all links
    pub link_predictions: Vec<LinkPrediction>,
}

/// Prediction for a single link
#[derive(Debug, Clone)]
pub struct LinkPrediction {
    /// Link identifier
    pub link_id: LinkId,
    /// Predicted SNR interval
    pub predicted_snr: crate::divergence::Interval,
    /// Predicted visibility
    pub predicted_visibility: bool,
}

/// Create channels for the digital twin runtime
#[allow(clippy::type_complexity)]
pub fn create_twin_channels() -> (
    mpsc::Sender<ObservedMetric>,
    mpsc::Receiver<ObservedMetric>,
    mpsc::Sender<AnomalySignal>,
    mpsc::Receiver<AnomalySignal>,
    watch::Sender<TwinForecast>,
    watch::Receiver<TwinForecast>,
) {
    let (observed_tx, observed_rx) = mpsc::channel(1000);
    let (anomaly_tx, anomaly_rx) = mpsc::channel(1000);
    let (forecast_tx, forecast_rx) = watch::channel(TwinForecast {
        forecast_time: Utc::now(),
        link_predictions: Vec::new(),
    });

    (
        observed_tx,
        observed_rx,
        anomaly_tx,
        anomaly_rx,
        forecast_tx,
        forecast_rx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_twin_channels() {
        let (observed_tx, mut observed_rx, _anomaly_tx, _anomaly_rx, _forecast_tx, _forecast_rx) =
            create_twin_channels();

        // Send an observed metric
        let link_id = uuid::Uuid::new_v4();
        let observed = ObservedMetric {
            link_id,
            metric_type: MetricType::Snr,
            value: 15.0,
            observation_time: Utc::now(),
        };
        observed_tx.send(observed).await.unwrap();

        // Receive it
        let received = observed_rx.recv().await.unwrap();
        assert_eq!(received.link_id, link_id);
        assert_eq!(received.value, 15.0);
    }
}

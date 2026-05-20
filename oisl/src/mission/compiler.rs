// Intent Compiler - translates declarative intent to imperative tasking

use crate::mission::state_machine::ConstellationState;
use crate::topology::{SpatiotemporalRouter, TopologyForecast};
use crate::{
    AssetId, BandwidthAllocation, BiTemporal, Bytes, ConfidenceScore, DataRate, GeoRegion, PlanId,
    Priority, SatelliteId, SensorType, TaskId, TenantId, TimeWindow,
};
use bitemporal::timestamp::{EventTime, ReceptionTime};
use chrono::Duration;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

// Types defined in compiler module
use super::{
    CompilationError, IntentConstraints, LinkReservation, MissionIntent, ObjectiveType,
    PlanExplanation, SatelliteTask, ServiceLevelAgreement, TaskType, TaskingPlan,
    ValidationWarning,
};

/// Intent compiler - compiles declarative intent to imperative tasking
pub trait IntentCompiler: Send + Sync {
    /// Compile intent to tasking plan
    fn compile(
        &self,
        intent: &MissionIntent,
        constellation: &ConstellationState,
        topology: &TopologyForecast,
    ) -> Result<TaskingPlan, CompilationError>;

    /// Validate a compiled plan
    fn validate(&self, plan: &TaskingPlan) -> Vec<ValidationWarning>;

    /// Explain why the compiler made specific decisions
    fn explain(&self, plan: &TaskingPlan) -> PlanExplanation;
}

/// Default intent compiler implementation
pub struct DefaultIntentCompiler {
    /// Wrapped in Mutex for interior mutability — the router caches routes
    /// internally but the compile trait interface requires &self.
    router: Mutex<SpatiotemporalRouter>,
}

impl DefaultIntentCompiler {
    pub fn new(router: SpatiotemporalRouter) -> Self {
        Self {
            router: Mutex::new(router),
        }
    }
}

impl IntentCompiler for DefaultIntentCompiler {
    fn compile(
        &self,
        intent: &MissionIntent,
        constellation: &ConstellationState,
        topology: &TopologyForecast,
    ) -> Result<TaskingPlan, CompilationError> {
        let plan_id = PlanId::new_v4();
        let now = Utc::now();
        let compiled_at = BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now));

        // Determine valid until based on intent deadline or default horizon
        let valid_until = intent
            .sla
            .deadline
            .unwrap_or_else(|| Utc::now() + Duration::hours(24));

        // Compile based on objective type
        let (satellite_tasks, link_reservations) = match &intent.objective {
            ObjectiveType::DataRelay {
                source,
                destination,
                volume,
            } => self.compile_data_relay(
                source,
                destination,
                *volume,
                constellation,
                topology,
                &intent.constraints,
                &intent.sla,
            )?,
            ObjectiveType::Observation {
                target,
                sensor,
                revisit_rate,
            } => self.compile_observation(
                target,
                sensor,
                *revisit_rate,
                constellation,
                topology,
                &intent.constraints,
                &intent.sla,
                &intent.submitted_by,
            )?,
            ObjectiveType::Custody {
                target,
                persistence,
            } => self.compile_custody(
                target,
                *persistence,
                constellation,
                topology,
                &intent.constraints,
                &intent.sla,
                &intent.submitted_by,
            )?,
            ObjectiveType::Downlink {
                satellite,
                ground_window,
            } => self.compile_downlink(
                satellite,
                ground_window,
                constellation,
                topology,
                &intent.constraints,
                &intent.sla,
                &intent.submitted_by,
            )?,
        };

        // Calculate confidence based on topology forecast quality
        let confidence = self.calculate_confidence(topology, &satellite_tasks);

        Ok(TaskingPlan {
            plan_id,
            parent_intent: intent.intent_id,
            satellite_tasks,
            link_reservations,
            compiled_at,
            valid_until,
            confidence,
        })
    }

    fn validate(&self, plan: &TaskingPlan) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Check for timing constraints
        for task in &plan.satellite_tasks {
            let task_duration = task.scheduled_window.duration();
            if task_duration < Duration::seconds(60) {
                warnings.push(ValidationWarning {
                    warning_type: crate::mission::ValidationWarningType::TimingMargin,
                    message: format!(
                        "Task {} has short duration window: {:?}",
                        task.task_id, task_duration
                    ),
                    severity: crate::mission::ValidationSeverity::Warning,
                });
            }
        }

        // Check for resource contention
        let terminal_usage: HashMap<_, Vec<_>> = plan
            .link_reservations
            .iter()
            .flat_map(|r| vec![&r.terminal_a, &r.terminal_b])
            .fold(HashMap::new(), |mut acc, term| {
                acc.entry(term).or_default().push(term);
                acc
            });

        for (terminal, usages) in terminal_usage {
            if usages.len() > 1 {
                warnings.push(ValidationWarning {
                    warning_type: crate::mission::ValidationWarningType::ResourceContention,
                    message: format!(
                        "Terminal {:?} has multiple concurrent reservations",
                        terminal
                    ),
                    severity: crate::mission::ValidationSeverity::Warning,
                });
            }
        }

        // Check plan confidence
        if plan.confidence.is_low() {
            warnings.push(ValidationWarning {
                warning_type: crate::mission::ValidationWarningType::LowConfidenceRoute,
                message: format!("Plan has low confidence: {:.2}", plan.confidence.0),
                severity: crate::mission::ValidationSeverity::Warning,
            });
        }

        warnings
    }

    fn explain(&self, plan: &TaskingPlan) -> PlanExplanation {
        // Generate explanation of routing decisions
        let routing_decisions = plan
            .satellite_tasks
            .iter()
            .filter_map(|task| {
                if let TaskType::OpticalLinkEstablishment {
                    peer_terminal: _, ..
                } = &task.task_type
                {
                    Some(crate::mission::RoutingDecision {
                        source: task.satellite_id.clone(),
                        destination: "peer".to_string(),
                        selected_route: vec![],
                        alternative_routes: vec![],
                        rationale: "Selected based on optimal geometry and availability"
                            .to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        PlanExplanation {
            summary: format!(
                "Plan {} compiled with {} tasks and {} link reservations",
                plan.plan_id,
                plan.satellite_tasks.len(),
                plan.link_reservations.len()
            ),
            routing_decisions,
            resource_allocations: vec![],
            tradeoffs: vec![],
            warnings: vec![],
        }
    }
}

impl DefaultIntentCompiler {
    fn compile_data_relay(
        &self,
        source: &AssetId,
        destination: &AssetId,
        volume: Bytes,
        constellation: &ConstellationState,
        topology: &TopologyForecast,
        constraints: &IntentConstraints,
        sla: &ServiceLevelAgreement,
    ) -> Result<(Vec<SatelliteTask>, Vec<LinkReservation>), CompilationError> {
        // Resolve source and destination to satellites
        let source_sat = constellation
            .resolve_asset_to_satellite(source)
            .ok_or_else(|| {
                CompilationError::InvalidIntent(format!(
                    "Source asset {} not found in constellation",
                    source
                ))
            })?;

        let dest_sat = constellation
            .resolve_asset_to_satellite(destination)
            .ok_or_else(|| {
                CompilationError::InvalidIntent(format!(
                    "Destination asset {} not found in constellation",
                    destination
                ))
            })?;

        // Compute route through constellation
        let route = self
            .router
            .lock()
            .map_err(|_| CompilationError::Internal("Router lock poisoned".to_string()))?
            .compute_route(&source_sat, &dest_sat, topology, constraints, sla)
            .map_err(|e| CompilationError::NoFeasibleRoute {
                source: source_sat.clone(),
                destination: dest_sat.clone(),
            })?;

        // Convert route to satellite tasks
        let mut satellite_tasks: Vec<SatelliteTask> = Vec::new();
        let mut link_reservations = Vec::new();

        for (i, hop) in route.hops.iter().enumerate() {
            let task_id = TaskId::new_v4();

            // Create optical link establishment task
            satellite_tasks.push(SatelliteTask {
                task_id,
                satellite_id: hop.edge.endpoints.0.clone(),
                task_type: TaskType::OpticalLinkEstablishment {
                    peer_terminal: constellation
                        .satellites
                        .get(&hop.edge.endpoints.1)
                        .and_then(|s| s.optical_terminals.first().copied())
                        .unwrap_or_else(|| {
                            Uuid::new_v5(&Uuid::NAMESPACE_DNS, hop.edge.endpoints.1.as_bytes())
                        }),
                    optical_config: crate::physical::OctConfiguration::default_s2s(),
                },
                scheduled_window: hop.use_window.clone(),
                dependencies: if i > 0 {
                    vec![satellite_tasks[i - 1].task_id]
                } else {
                    vec![]
                },
                priority: Priority::Medium,
                tenant_id: source.clone(),
            });

            // Resolve satellite node IDs to terminal UUIDs for the link reservation.
            // Fall back to deterministic UUIDs derived from node IDs if the
            // constellation doesn't have explicit terminal mappings yet.
            let term_a = constellation
                .satellites
                .get(&hop.edge.endpoints.0)
                .and_then(|s| s.optical_terminals.first().copied())
                .unwrap_or_else(|| {
                    Uuid::new_v5(&Uuid::NAMESPACE_DNS, hop.edge.endpoints.0.as_bytes())
                });
            let term_b = constellation
                .satellites
                .get(&hop.edge.endpoints.1)
                .and_then(|s| s.optical_terminals.first().copied())
                .unwrap_or_else(|| {
                    Uuid::new_v5(&Uuid::NAMESPACE_DNS, hop.edge.endpoints.1.as_bytes())
                });

            link_reservations.push(LinkReservation {
                reservation_id: Uuid::new_v4(),
                terminal_a: term_a,
                terminal_b: term_b,
                time_window: hop.use_window.clone(),
                bandwidth_allocation: hop.bandwidth_reservation.clone(),
                task_ids: vec![task_id],
            });
        }

        Ok((satellite_tasks, link_reservations))
    }

    fn compile_observation(
        &self,
        target: &GeoRegion,
        sensor: &SensorType,
        revisit_rate: Duration,
        constellation: &ConstellationState,
        topology: &TopologyForecast,
        constraints: &IntentConstraints,
        sla: &ServiceLevelAgreement,
        tenant_id: &TenantId,
    ) -> Result<(Vec<SatelliteTask>, Vec<LinkReservation>), CompilationError> {
        let mut satellite_tasks = Vec::new();
        let mut link_reservations = Vec::new();

        // Schedule observation windows across the topology horizon.
        // Step through the forecast interval at each revisit_rate tick.
        let horizon_seconds = topology
            .horizon_end
            .signed_duration_since(topology.horizon_start)
            .num_seconds();
        let revisit_secs = revisit_rate.num_seconds().max(60);
        let num_windows = (horizon_seconds / revisit_secs).max(1) as usize;

        // Find candidate satellites (all satellites in the constellation for now;
        // a future enhancement would filter by orbital visibility over the target).
        let candidate_sats: Vec<&SatelliteId> = constellation.satellites.keys().collect();
        if candidate_sats.is_empty() {
            return Err(CompilationError::NoFeasibleRoute {
                source: "observation-asset".to_string(),
                destination: target.name.clone(),
            });
        }

        for window_idx in 0..num_windows {
            let window_start =
                topology.horizon_start + Duration::seconds(window_idx as i64 * revisit_secs);
            let window_end = window_start + Duration::seconds(revisit_secs.min(600)); // cap at 10-min passes
            let obs_window = TimeWindow::new(window_start, window_end);

            // Round-robin across candidate satellites for revisit coverage
            let sat_id: SatelliteId = candidate_sats[window_idx % candidate_sats.len()].clone();
            let task_id = TaskId::new_v4();

            satellite_tasks.push(SatelliteTask {
                task_id,
                satellite_id: sat_id.clone(),
                task_type: TaskType::Observation {
                    target: target.clone(),
                    sensor: sensor.clone(),
                },
                scheduled_window: obs_window.clone(),
                dependencies: vec![],
                priority: sla.priority,
                tenant_id: tenant_id.clone(),
            });

            // After each observation window, schedule a downlink to deliver the data.
            let downlink_start = window_end;
            let downlink_end = downlink_start + Duration::seconds(300); // 5-min downlink
            let downlink_window = TimeWindow::new(downlink_start, downlink_end);

            if let Some(gs_id) =
                constellation.find_optimal_ground_station(&sat_id, &downlink_window, constraints)
            {
                let dl_task_id = TaskId::new_v4();
                let gs_terminal = Uuid::new_v5(&Uuid::NAMESPACE_DNS, gs_id.as_bytes());
                let sat_terminal = constellation
                    .satellites
                    .get(&sat_id)
                    .and_then(|s| s.optical_terminals.first().copied())
                    .unwrap_or_else(|| Uuid::new_v5(&Uuid::NAMESPACE_DNS, sat_id.as_bytes()));

                satellite_tasks.push(SatelliteTask {
                    task_id: dl_task_id,
                    satellite_id: sat_id.clone(),
                    task_type: TaskType::Downlink {
                        ground_station: gs_id,
                    },
                    scheduled_window: downlink_window.clone(),
                    dependencies: vec![task_id],
                    priority: sla.priority,
                    tenant_id: tenant_id.clone(),
                });

                link_reservations.push(LinkReservation {
                    reservation_id: Uuid::new_v4(),
                    terminal_a: sat_terminal,
                    terminal_b: gs_terminal,
                    time_window: downlink_window,
                    bandwidth_allocation: BandwidthAllocation {
                        data_rate: DataRate(1_000_000), // 1 Mbps default
                        valid_window: obs_window,
                    },
                    task_ids: vec![task_id, dl_task_id],
                });
            }
        }

        Ok((satellite_tasks, link_reservations))
    }

    fn compile_custody(
        &self,
        target: &AssetId,
        persistence: Duration,
        constellation: &ConstellationState,
        topology: &TopologyForecast,
        _constraints: &IntentConstraints,
        sla: &ServiceLevelAgreement,
        tenant_id: &TenantId,
    ) -> Result<(Vec<SatelliteTask>, Vec<LinkReservation>), CompilationError> {
        // Resolve target asset to a satellite (the asset being "kept in custody")
        let target_sat = constellation
            .resolve_asset_to_satellite(target)
            .ok_or_else(|| {
                CompilationError::InvalidIntent(format!(
                    "Custody target asset '{}' not found in constellation",
                    target
                ))
            })?;

        let mut satellite_tasks = Vec::new();
        let mut link_reservations = Vec::new();

        // Custody = maintain an optical relay chain to the target asset for the
        // full persistence duration.  We schedule overlapping custody windows so
        // there is always at least one satellite tracking the target.
        let horizon_seconds = topology
            .horizon_end
            .signed_duration_since(topology.horizon_start)
            .num_seconds()
            .min(persistence.num_seconds());
        let segment_secs = 600_i64; // 10-min custody segments with 60-s overlap
        let overlap_secs = 60_i64;
        let num_segments = ((horizon_seconds + segment_secs - 1) / segment_secs).max(1) as usize;

        // Candidate relay satellites (all except the target itself)
        let relay_sats: Vec<&SatelliteId> = constellation
            .satellites
            .keys()
            .filter(|id| *id != &target_sat)
            .collect();

        if relay_sats.is_empty() {
            return Err(CompilationError::NoFeasibleRoute {
                source: target_sat.clone(),
                destination: "custody-relay".to_string(),
            });
        }

        let mut prev_task_id: Option<TaskId> = None;

        for seg_idx in 0..num_segments {
            // Start each segment `overlap_secs` before the previous one ends
            // so there is always a satellite ready to hand over.
            let seg_start = topology.horizon_start
                + Duration::seconds(seg_idx as i64 * (segment_secs - overlap_secs));
            let seg_end = seg_start + Duration::seconds(segment_secs);
            let seg_window = TimeWindow::new(seg_start, seg_end);

            let relay_sat = relay_sats[seg_idx % relay_sats.len()].clone();
            let task_id = TaskId::new_v4();

            // Task the relay satellite to maintain an optical link to the target
            satellite_tasks.push(SatelliteTask {
                task_id,
                satellite_id: relay_sat.clone(),
                task_type: TaskType::OpticalLinkEstablishment {
                    peer_terminal: constellation
                        .satellites
                        .get(&target_sat)
                        .and_then(|s| s.optical_terminals.first().copied())
                        .unwrap_or_else(|| {
                            Uuid::new_v5(&Uuid::NAMESPACE_DNS, target_sat.as_bytes())
                        }),
                    optical_config: crate::physical::OctConfiguration::default_s2s(),
                },
                scheduled_window: seg_window.clone(),
                dependencies: prev_task_id.map(|id| vec![id]).unwrap_or_default(),
                priority: sla.priority,
                tenant_id: tenant_id.clone(),
            });

            // Reserve the S2S link between relay and target
            let relay_terminal = constellation
                .satellites
                .get(&relay_sat)
                .and_then(|s| s.optical_terminals.first().copied())
                .unwrap_or_else(|| Uuid::new_v5(&Uuid::NAMESPACE_DNS, relay_sat.as_bytes()));
            let target_terminal = constellation
                .satellites
                .get(&target_sat)
                .and_then(|s| s.optical_terminals.first().copied())
                .unwrap_or_else(|| Uuid::new_v5(&Uuid::NAMESPACE_DNS, target_sat.as_bytes()));

            link_reservations.push(LinkReservation {
                reservation_id: Uuid::new_v4(),
                terminal_a: relay_terminal,
                terminal_b: target_terminal,
                time_window: seg_window.clone(),
                bandwidth_allocation: BandwidthAllocation {
                    data_rate: DataRate(10_000_000), // 10 Mbps S2S
                    valid_window: seg_window,
                },
                task_ids: vec![task_id],
            });

            prev_task_id = Some(task_id);
        }

        Ok((satellite_tasks, link_reservations))
    }

    fn compile_downlink(
        &self,
        satellite: &SatelliteId,
        ground_window: &TimeWindow,
        constellation: &ConstellationState,
        _topology: &TopologyForecast,
        constraints: &IntentConstraints,
        sla: &ServiceLevelAgreement,
        tenant_id: &TenantId,
    ) -> Result<(Vec<SatelliteTask>, Vec<LinkReservation>), CompilationError> {
        let ground_station = constellation
            .find_optimal_ground_station(satellite, ground_window, constraints)
            .ok_or_else(|| CompilationError::NoFeasibleRoute {
                source: satellite.clone(),
                destination: "ground".to_string(),
            })?;

        let task_id = TaskId::new_v4();

        let satellite_tasks = vec![SatelliteTask {
            task_id,
            satellite_id: satellite.clone(),
            task_type: TaskType::Downlink {
                ground_station: ground_station.clone(),
            },
            scheduled_window: ground_window.clone(),
            dependencies: vec![],
            priority: sla.priority,
            tenant_id: tenant_id.clone(),
        }];

        // Reserve the S2T (satellite-to-ground) optical link for the downlink window.
        let sat_terminal = constellation
            .satellites
            .get(satellite)
            .and_then(|s| s.optical_terminals.first().copied())
            .unwrap_or_else(|| Uuid::new_v5(&Uuid::NAMESPACE_DNS, satellite.as_bytes()));
        let gs_terminal = Uuid::new_v5(&Uuid::NAMESPACE_DNS, ground_station.as_bytes());

        let link_reservations = vec![LinkReservation {
            reservation_id: Uuid::new_v4(),
            terminal_a: sat_terminal,
            terminal_b: gs_terminal,
            time_window: ground_window.clone(),
            bandwidth_allocation: BandwidthAllocation {
                data_rate: DataRate(100_000_000), // 100 Mbps S2T optical downlink
                valid_window: ground_window.clone(),
            },
            task_ids: vec![task_id],
        }];

        Ok((satellite_tasks, link_reservations))
    }

    fn calculate_confidence(
        &self,
        topology: &TopologyForecast,
        _tasks: &[SatelliteTask],
    ) -> ConfidenceScore {
        // Confidence based on forecast horizon and quality
        let horizon = topology
            .horizon_end
            .signed_duration_since(topology.horizon_start);
        let base_confidence = if horizon > Duration::hours(12) {
            0.7
        } else if horizon > Duration::hours(6) {
            0.85
        } else {
            0.95
        };

        ConfidenceScore::new(base_confidence)
    }
}

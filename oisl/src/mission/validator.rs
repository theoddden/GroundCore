// Plan Validator - runs validation passes before commit

use crate::mission::{TaskingPlan, ValidationWarning, ValidationSeverity, ValidationWarningType};
use crate::mission::state_machine::ConstellationState;
use crate::topology::TopologyForecast;
use std::collections::HashMap;

/// Plan validator
pub struct PlanValidator {
    constellation: ConstellationState,
    topology: TopologyForecast,
}

impl PlanValidator {
    pub fn new(constellation: ConstellationState, topology: TopologyForecast) -> Self {
        Self {
            constellation,
            topology,
        }
    }

    /// Validate a complete tasking plan
    pub fn validate(&self, plan: &TaskingPlan) -> ValidationReport {
        let mut warnings = Vec::new();

        // Run validation passes
        warnings.extend(self.validate_regulatory_compliance(plan));
        warnings.extend(self.validate_sla_compliance(plan));
        warnings.extend(self.validate_resource_availability(plan));
        warnings.extend(self.validate_topology_consistency(plan));
        warnings.extend(self.validate_timing_constraints(plan));

        ValidationReport {
            is_valid: !warnings.iter().any(|w| w.severity == ValidationSeverity::Error),
            warnings,
        }
    }

    fn validate_regulatory_compliance(&self, plan: &TaskingPlan) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Check for operations in restricted regions
        for task in &plan.satellite_tasks {
            if let crate::mission::TaskType::Observation { target, .. } = &task.task_type {
                // TODO: Check against regulatory database
                if self.is_restricted_region(target) {
                    warnings.push(ValidationWarning {
                        warning_type: ValidationWarningType::RegulatoryConcern,
                        message: format!("Observation target may be in restricted region: {:?}", target),
                        severity: ValidationSeverity::Error,
                    });
                }
            }
        }

        warnings
    }

    fn validate_sla_compliance(&self, plan: &TaskingPlan) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Check if plan deadline can be met
        let plan_end = plan.satellite_tasks.iter()
            .map(|t| t.scheduled_window.end)
            .max()
            .unwrap_or(plan.valid_until);

        // This would need to be compared against the parent intent's deadline
        // For now, just check if plan exceeds valid_until
        if plan_end > plan.valid_until {
            warnings.push(ValidationWarning {
                warning_type: ValidationWarningType::TimingMargin,
                message: format!("Plan extends beyond valid_until: {:?} > {:?}", plan_end, plan.valid_until),
                severity: ValidationSeverity::Error,
            });
        }

        warnings
    }

    fn validate_resource_availability(&self, plan: &TaskingPlan) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Count terminal usage
        let mut terminal_usage: HashMap<crate::TerminalId, usize> = HashMap::new();
        
        for reservation in &plan.link_reservations {
            *terminal_usage.entry(reservation.terminal_a.clone()).or_insert(0) += 1;
            *terminal_usage.entry(reservation.terminal_b.clone()).or_insert(0) += 1;
        }

        // Check for over-subscribed terminals
        for (terminal, count) in terminal_usage {
            if count > 1 {
                warnings.push(ValidationWarning {
                    warning_type: ValidationWarningType::ResourceContention,
                    message: format!("Terminal {:?} used {} times concurrently", terminal, count),
                    severity: ValidationSeverity::Warning,
                });
            }
        }

        warnings
    }

    fn validate_topology_consistency(&self, plan: &TaskingPlan) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Verify that all link reservations are within topology forecast
        for reservation in &plan.link_reservations {
            let snapshot = self.topology.query_at(reservation.time_window.start);
            
            // Check if terminals exist in topology
            let terminal_a_exists = snapshot.nodes.contains_key(&crate::NodeId::from(reservation.terminal_a.to_string()));
            let terminal_b_exists = snapshot.nodes.contains_key(&crate::NodeId::from(reservation.terminal_b.to_string()));

            if !terminal_a_exists || !terminal_b_exists {
                warnings.push(ValidationWarning {
                    warning_type: ValidationWarningType::LowConfidenceRoute,
                    message: format!("Link reservation references terminal not in topology"),
                    severity: ValidationSeverity::Error,
                });
            }
        }

        warnings
    }

    fn validate_timing_constraints(&self, plan: &TaskingPlan) -> Vec<ValidationWarning> {
        let mut warnings = Vec::new();

        // Check for overlapping tasks on same satellite
        let mut satellite_timings: HashMap<crate::SatelliteId, Vec<&crate::mission::TimeWindow>> = HashMap::new();

        for task in &plan.satellite_tasks {
            satellite_timings
                .entry(task.satellite_id.clone())
                .or_default()
                .push(&task.scheduled_window);
        }

        for (satellite, windows) in satellite_timings {
            for (i, w1) in windows.iter().enumerate() {
                for w2 in windows.iter().skip(i + 1) {
                    if w1.overlaps(w2) {
                        warnings.push(ValidationWarning {
                            warning_type: ValidationWarningType::ResourceContention,
                            message: format!("Satellite {} has overlapping task windows", satellite),
                            severity: ValidationSeverity::Warning,
                        });
                    }
                }
            }
        }

        warnings
    }

    fn is_restricted_region(&self, _target: &crate::GeoRegion) -> bool {
        // TODO: Implement actual regulatory check
        false
    }
}

/// Validation report
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub warnings: Vec<ValidationWarning>,
}

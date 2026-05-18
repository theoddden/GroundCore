// Satellite Node Manager - per-satellite resource state

use crate::{
    SatelliteId, TerminalId, TaskId, TenantId,
    DataRate, Bytes, TimeWindow, Priority,
};
use crate::resource::{ResourceAllocation, ResourceClaim};
use crate::physical::terminal::TerminalCapability as PhysicalTerminalCapability;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Satellite node - distributed system node with bounded resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteNode {
    pub satellite_id: SatelliteId,
    
    // Physical capabilities
    pub optical_terminals: HashMap<TerminalId, TerminalCapability>,
    pub rf_terminals: HashMap<TerminalId, RfCapability>,
    pub sensors: HashMap<String, SensorCapability>,
    
    // Compute resources (next-gen capability)
    pub compute: ComputeResources,
    pub storage: StorageResources,
    
    // Power and thermal
    pub power_budget: PowerBudget,
    pub thermal_state: ThermalState,
    
    // Mission state
    pub current_allocations: Vec<ResourceAllocation>,
    pub pending_tasks: VecDeque<TaskId>,
    
    // Health
    pub health_metrics: HealthMetrics,
    pub degradation_predictions: Vec<DegradationForecast>,
}

/// Terminal capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalCapability {
    pub terminal_id: TerminalId,
    pub vendor: crate::physical::Vendor,
    pub model: String,
    pub max_data_rate: DataRate,
    pub max_pointing_accuracy_rad: f64,
    pub power_consumption_watts: f64,
    pub available: bool,
}

/// RF capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfCapability {
    pub terminal_id: TerminalId,
    pub frequency_bands: Vec<crate::Frequency>,
    pub max_data_rate: DataRate,
    pub power_consumption_watts: f64,
}

/// Sensor capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorCapability {
    pub sensor_type: crate::SensorType,
    pub resolution: String,
    pub swath_width_km: f64,
    pub power_consumption_watts: f64,
}

/// Compute resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResources {
    pub cpu_cores: u32,
    pub memory_gb: f64,
    pub gpu_available: bool,
    pub allocated_cpu: f64,
    pub allocated_memory: f64,
}

/// Storage resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageResources {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

/// Power budget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerBudget {
    pub total_watts: f64,
    pub allocated_watts: f64,
    pub available_watts: f64,
    pub battery_capacity_wh: f64,
    pub battery_charge_percent: f64,
}

/// Thermal state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalState {
    pub current_temperature_c: f64,
    pub max_temperature_c: f64,
    pub thermal_margin_c: f64,
}

/// Health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub overall_health: f64, // 0.0 to 1.0
    pub last_check: DateTime<Utc>,
    pub component_health: HashMap<String, f64>,
}

/// Degradation forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationForecast {
    pub component: String,
    pub predicted_degradation_time: DateTime<Utc>,
    pub confidence: f64,
    pub recommended_mitigation: String,
}

impl SatelliteNode {
    pub fn new(satellite_id: SatelliteId) -> Self {
        Self {
            satellite_id,
            optical_terminals: HashMap::new(),
            rf_terminals: HashMap::new(),
            sensors: HashMap::new(),
            compute: ComputeResources::default(),
            storage: StorageResources::default(),
            power_budget: PowerBudget::default(),
            thermal_state: ThermalState::default(),
            current_allocations: vec![],
            pending_tasks: VecDeque::new(),
            health_metrics: HealthMetrics::default(),
            degradation_predictions: vec![],
        }
    }

    /// Add optical terminal
    pub fn add_optical_terminal(&mut self, capability: TerminalCapability) {
        self.optical_terminals.insert(capability.terminal_id.clone(), capability);
    }

    /// Check if resource claim can be satisfied
    pub fn can_satisfy(&self, claim: &ResourceClaim) -> bool {
        // Check power availability
        if claim.power_watts > self.power_budget.available_watts {
            return false;
        }

        // Check terminal availability
        if let Some(terminal_id) = &claim.optical_terminal {
            let terminal = self.optical_terminals.get(terminal_id);
            match terminal {
                Some(cap) if cap.available => {},
                _ => return false,
            }
        }

        // Check compute availability
        if let Some(cycles) = claim.compute_cycles {
            if cycles > (self.compute.cpu_cores as u64 * 1e9) {
                return false;
            }
        }

        // Check storage availability
        if let Some(bytes) = claim.storage_bytes {
            if bytes > self.storage.available_bytes {
                return false;
            }
        }

        true
    }

    /// Allocate resources for a claim
    pub fn allocate(&mut self, claim: ResourceClaim, task_id: TaskId, tenant_id: TenantId) -> ResourceAllocation {
        let allocation_id = crate::AllocationId::new_v4();
        
        // Update resource usage
        self.power_budget.allocated_watts += claim.power_watts;
        self.power_budget.available_watts -= claim.power_watts;

        if let Some(cycles) = claim.compute_cycles {
            self.compute.allocated_cpu += cycles as f64 / 1e9;
        }

        if let Some(bytes) = claim.storage_bytes {
            self.storage.used_bytes += bytes;
            self.storage.available_bytes -= bytes;
        }

        let allocation = ResourceAllocation {
            allocation_id,
            task_id,
            tenant_id,
            resources: claim,
            valid_window: TimeWindow::new(Utc::now(), Utc::now() + Duration::hours(1)),
            priority: Priority::Medium,
            preemptible: true,
        };

        self.current_allocations.push(allocation.clone());
        allocation
    }

    /// Release allocated resources
    pub fn release(&mut self, allocation_id: &crate::AllocationId) {
        if let Some(pos) = self.current_allocations.iter().position(|a| &a.allocation_id == allocation_id) {
            let allocation = self.current_allocations.remove(pos);
            
            // Restore resources
            self.power_budget.allocated_watts -= allocation.resources.power_watts;
            self.power_budget.available_watts += allocation.resources.power_watts;

            if let Some(cycles) = allocation.resources.compute_cycles {
                self.compute.allocated_cpu -= cycles as f64 / 1e9;
            }

            if let Some(bytes) = allocation.resources.storage_bytes {
                self.storage.used_bytes -= bytes;
                self.storage.available_bytes += bytes;
            }
        }
    }

    /// Get available bandwidth
    pub fn available_bandwidth(&self) -> DataRate {
        let total_capacity: u64 = self.optical_terminals.values()
            .filter(|t| t.available)
            .map(|t| t.max_data_rate.0)
            .sum();
        
        let allocated: u64 = self.current_allocations.iter()
            .filter_map(|a| a.resources.bandwidth)
            .map(|b| b.0)
            .sum();

        DataRate(total_capacity.saturating_sub(allocated))
    }

    /// Add pending task
    pub fn add_pending_task(&mut self, task_id: TaskId) {
        self.pending_tasks.push_back(task_id);
    }

    /// Pop next pending task
    pub fn pop_pending_task(&mut self) -> Option<TaskId> {
        self.pending_tasks.pop_front()
    }
}

impl Default for ComputeResources {
    fn default() -> Self {
        Self {
            cpu_cores: 4,
            memory_gb: 16.0,
            gpu_available: false,
            allocated_cpu: 0.0,
            allocated_memory: 0.0,
        }
    }
}

impl Default for StorageResources {
    fn default() -> Self {
        Self {
            total_bytes: 1_000_000_000_000, // 1 TB
            used_bytes: 0,
            available_bytes: 1_000_000_000_000,
        }
    }
}

impl Default for PowerBudget {
    fn default() -> Self {
        Self {
            total_watts: 500.0,
            allocated_watts: 0.0,
            available_watts: 500.0,
            battery_capacity_wh: 2000.0,
            battery_charge_percent: 100.0,
        }
    }
}

impl Default for ThermalState {
    fn default() -> Self {
        Self {
            current_temperature_c: 20.0,
            max_temperature_c: 50.0,
            thermal_margin_c: 30.0,
        }
    }
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            overall_health: 1.0,
            last_check: Utc::now(),
            component_health: HashMap::new(),
        }
    }
}

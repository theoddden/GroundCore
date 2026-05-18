# Ground Core

A 100% Rust implementation of satellite ground station protocols supporting both RF and optical communications with advanced architectural patterns for reliability, security, and performance.

## Architecture

Ground Core is organized into two complementary communication layers:

### RF Communications Layer
- **RF processing** with no memory allocations for microseconds-level latency
- **Doppler correction** using predictive NCO programming with phase-continuous updates
- **Shadow-tracking SDRs** for lossless failover (<100ms switchover)
- **Demodulator state snapshotting** for recovery across failures

### Optical Communications Layer
- **PAT (Pointing, Acquisition, Tracking)** with precision clock synchronization
- **Geometry computations** for pointing vectors, visibility windows, Doppler shift, and atmospheric attenuation
- **Link state machine** managing link phases, quality, degradation, recovery, and failure
- **Terminal abstraction** via traits for vendor interoperability (Mynaric, Tesat)
- **Cryptographic attestation** for link and PAT events with bi-temporal timestamps
- **Spatiotemporal routing** over time-varying topology graphs
- **Mission planning** with declarative intents compiled into imperative tasking plans

### Non-Real-Time Management Layer
- **Satellite tracking** with SGP4 propagation and UKF refinement
- **Hardware management** with pass-isolated shards for failure containment
- **Scheduler** with Dominant Resource Fairness (DRF) and reputation-weighted fairness
- **Regulatory compliance** enforced at compile-time via type system
- **Federation layer** with cryptographic attestation for cross-station coordination
- **Resource allocation** with multi-tenant isolation and preemption support

### Human-Facing Layer
- **Embedded Claude Code agent** for anomaly detection and decision explanation
- **REST/gRPC API** for external integration
- **Web UI** (Leptos) for operator interaction

## Nine Problems and Solutions

### Problem 1: Lossless Failover
**Solution:** Shadow-tracking SDRs with pointer swap on failure. Demodulator state snapshots enable instant recovery without sample loss.

### Problem 2: Doppler Correction Without Sample Loss
**Solution:** Predictive NCO programming using SGP4 propagation and UKF refinement. Phase-continuous frequency updates prevent retuning artifacts.

### Problem 3: Fair Scheduling with Adversarial Tenants
**Solution:** Dominant Resource Fairness (DRF) over dominant resources, not a single shared resource. Reputation weighting prioritizes tenants who use what they're allocated.

### Problem 4: Federation with Adversarial Peers
**Solution:** Periodic challenge passes with cryptographic attestation. Trust scores computed from cross-verification history. Bi-temporal logging provides provable verification.

### Problem 5: Spectrum Compliance as Types
**Solution:** Regulatory rules compiled to Rust types. Transmit functions require a license typed to the specific band. Violations caught at compile time, not runtime.

### Problem 6: Continuous Operation Through Code Deployment
**Solution:** Pass-isolated process supervision. Each pass runs in its own subprocess. Deployment swaps the parent; passes continue under old code until completion.

### Problem 7: Synchronized PAT Acquisition Across Satellites
**Solution:** Precision clock synchronization with GPS-disciplined oscillators (~50ns accuracy). Bi-temporal logging proves timestamps were agreed correctly. Clock confidence tracking prevents acquisition when clocks are degraded.

### Problem 8: Spatiotemporal Routing Over Time-Varying Topology
**Solution:** Time-indexed graph forecasting with configurable resolution. Modified Dijkstra algorithm for routing through both space and time. Route caching with 5-minute time buckets for performance.

### Problem 9: Vendor Interoperability for Optical Terminals
**Solution:** Trait-based abstraction over SDA OCT Standard. Mynaric CONDOR Mk3 and Tesat SCOT80 implementations share the same OpticalTerminal trait while handling vendor-specific control protocols.

## Four Architectural Primitives

### Sharding (Foundational)
- **Per-pass shards:** Each pass gets isolated memory region using `bumpalo` for no-heap allocations
- **Emergency shards:** Pre-allocated memory regions for failover
- **Tenant shards:** Cryptographic isolation of tenant data with protected memory regions

- Multi-tenant isolation is enforced at kernel level, not soft access control
- Zero-downtime deployment is trivial: new code starts new shards, old shards finish under old code
- Failover is contained: one pass's failure can't cascade due to no shared mutable state

### Caching
- **Orbital propagation:** Interpolate between cached positions (sub-meter accuracy without re-running SGP4)
- **TLE refresh:** Atomic batch refresh for consistent constellation state
- **Schedule fragments:** Incremental re-optimization in <500ms instead of full annealing

### Batching
- **TLE refresh:** Atomic constellation state updates
- **Demodulator output:** Batch symbols for efficient Float Protocols delivery
- **Log entries:** Batch for efficient storage

### Snapshotting
- **Schedule snapshots:** For incremental re-optimization
- **Demodulator state:** For failover recovery
- **Federation exchange:** For cross-station verification

## Project Structure

```
ground-station-core/
├── Cargo.toml              # Workspace configuration
├── core/                   # Central types and error handling
├── optical/                # Optical communications layer
│   ├── pat/               # Pointing, Acquisition, Tracking
│   ├── geometry/          # Geometry computations
│   ├── link/              # Link lifecycle management
│   ├── terminal/          # Terminal abstraction
│   ├── attestation/       # Cryptographic attestation
│   └── routing/           # Optical routing
├── oisl/                   # Optical Inter-Satellite Link control plane
│   ├── mission/           # Mission planning and compilation
│   ├── topology/          # Spatiotemporal topology forecasting
│   ├── resource/          # Distributed resource management
│   ├── physical/          # Vendor abstraction (SDA OCT)
│   ├── pat/               # Synchronized PAT coordination
│   └── federation/        # Cross-operator OISL coordination
├── rf-layer/              # Real-time RF processing
├── tracking/             # Satellite tracking (SGP4 + UKF)
├── hardware/              # Hardware management with sharding
├── scheduler/            # DRF scheduler with reputation weighting
├── bitemporal/           # Bi-temporal logging with provenance
├── sdr-sim/              # Simulated SDR for testing
├── policy-compiler/       # Spectrum policy → Rust types
├── regulatory/           # Compile-time regulatory enforcement
├── federation/           # Cryptographic attestation for federation
├── agent/                # Embedded Claude Code agent
├── deployment/           # Zero-downtime process supervision
├── caching/              # Caching infrastructure
├── batching/             # Batching infrastructure
└── snapshotting/         # Snapshotting infrastructure
```

## Key Crates

### Optical Communications (`optical/`)

#### PAT Module (`optical/pat/`)
- `TrackingQuality` - Metrics for pointing error, SNR, and lock confidence
- `TrackingMetrics` - Historical tracking data with health checks
- `TrackingHandle` - Active tracking session management
- `LossReason` - Classification of acquisition loss causes
- `RecoveryStrategy` - Mapping from loss reasons to recovery actions
- `FallbackAction` - Actions for failed acquisition (retry, use alternative terminal, abort)
- `RecoveryAttempt` - Tracking recovery attempts with timestamps and success status

#### Geometry Module (`optical/geometry/`)
- `PointingVector` - Azimuth/elevation/range with spherical/cartesian conversion
- `SatellitePosition` - 3D position with velocity for range rate computation
- `compute_pointing_vector()` - Calculate pointing vector from observer to satellite
- `compute_relative_velocity()` - Calculate relative velocity between satellites
- `compute_range_rate()` - Calculate Doppler-based range rate
- `VisibilityWindow` - Time window with geometric constraints
- `SolarExclusionZone` - Sun avoidance constraints
- `ForbiddenZone` - Geographic or angular exclusion zones
- `predict_visibility_windows()` - Simplified visibility prediction based on orbit
- `DopplerPrediction` - Optical Doppler shift calculations
- `compute_optical_doppler()` - Doppler shift from relative velocity and wavelength
- `AtmosphericPath` - Path through atmosphere with elevation angle
- `WeatherConditions` - Humidity, temperature, cloud cover, visibility
- `sample_weather_profiles()` - Pre-defined weather profiles for testing
- `compute_attenuation()` - Atmospheric attenuation in dB based on wavelength and conditions
- `ATTENUATION_ACCEPTABLE_THRESHOLD` - 3 dB threshold for acceptable link quality
- `ATTENUATION_CRITICAL_THRESHOLD` - 10 dB threshold for critical degradation

#### Link Module (`optical/link/`)
- `OpticalLink` - State machine for link lifecycle
- `LinkPhase` - Phases: Scheduled, Acquiring, Established, Degrading, Recovering, Terminating, Failed
- `LinkQuality` - BER, SNR, pointing error metrics
- `DegradationReason` - Atmospheric turbulence, thermal stress, power limitation, etc.
- `RecoveryStrategy` - Handoff, monitoring, power cycling
- `TerminationReason` - Normal, timeout, preemption, error
- `FailureCause` - Hardware failure, network failure, clock desync
- `MetricSnapshot` - Link metrics at a specific timestamp
- `BoundedHistory` - Fixed-size rolling history of metric snapshots
- `compute_average_ber()` - Average BER over history
- `detect_trend()` - Detect Improving, Stable, or Degrading trends
- `DegradationDetector` - Detect degradation based on thresholds
- `detect_degradation()` - Check recent metrics against thresholds
- `classify_degradation()` - Determine degradation reason and suggest action
- `DegradationEvent` - Record of detected degradation
- `FailoverStrategy` - Immediate, graceful, or none
- `HandoffStrategy` - Pre-emptive, reactive, or predictive
- `HandoffExecution` - Track handoff progress with timing
- `FailoverManager` - Initiate and complete handoffs with error handling
- `initiate_handoff()` - Start handoff to alternative link
- `complete_handoff()` - Finalize handoff and update state

#### Terminal Module (`optical/terminal/`)
- `TerminalCapability` - Vendor, model, data rate, elevation, field of regard, power profile, standards, max range
- `FieldOfRegard` - Azimuth and elevation ranges
- `PowerProfile` - Power consumption by state (idle, tracking, transmitting)
- `OPTICAL_TERMINAL_CAPABILITIES` - Pre-defined capabilities for specific models
- `OpticalTerminal` (trait) - Async trait for vendor interoperability
  - `identity()` - Get terminal identity and configuration
  - `configure_pat()` - Configure PAT parameters
  - `start_tracking()` - Begin tracking with handle
  - `stop_tracking()` - Stop tracking session
  - `get_data_stream()` - Get data channel
  - `get_telemetry_stream()` - Get telemetry channel
  - `get_health_report()` - Get health status
  - `enter_safe_mode()` - Enter safe mode
  - `reset()` - Reset terminal
- `TrackingHandle` - Handle for active tracking session
- `TrackingMetrics` - Metrics from tracking session
- `TelemetryFrame` - Single telemetry frame with type and data
- `TelemetryType` - Position, signal, thermal, power, custom
- `TelemetryStream` (trait) - Async stream of telemetry frames
- `batch_frames()` - Batch multiple frames for efficiency
- `HealthReport` - Component health and overall status
- `ComponentHealth` - Health status for individual components
- `SystemStatus` - Overall system status
- `ResetLevel` - Soft, Hard, or Factory reset
- `SafeModeState` - Normal, Degraded, Safe
- `RecoveryProcedure` - Steps for safe mode recovery
- `RecoveryStep` - Individual recovery step with action and timeout

#### Attestation Module (`optical/attestation/`)
- `TerminalIdentity` - Terminal identification information
- `IdentityCertificate` - Cryptographic certificate for terminal identity
- `create_certificate()` - Create identity certificate
- `validate_certificate()` - Validate certificate signature and expiration
- `Hash` - Cryptographic hash type
- `Signature` - Cryptographic signature type
- `AttestableEvent` - Event that can be cryptographically attested
- `LinkAttestation` - Cryptographic proof of link establishment
- `create_link_attestation()` - Create attestation for link event
- `verify_link_attestation()` - Verify attestation signature and hash
- `PatAttestation` - Attestation for PAT events
- `AcquisitionProof` - Proof of acquisition with bi-temporal timestamps and metrics
- `TrackingProof` - Proof of tracking with bi-temporal timestamps and quality
- `create_acquisition_proof()` - Create proof for acquisition event
- `augment_acquisition_proof()` - Add metrics to acquisition proof
- `create_tracking_proof()` - Create proof for tracking event
- `augment_tracking_proof()` - Add quality metrics to tracking proof

#### Routing Module (`optical/routing/`)
- `OpticalTopology` - Graph of nodes (satellites) and edges (links)
- `OpticalNode` - Satellite with position and terminals
- `SatellitePosition` - 3D position with velocity
- `TopologyEdge` - Edge with visibility window and pointing vector
- `add_node()` - Add satellite to topology
- `find_node()` - Find satellite by ID
- `add_edge()` - Add potential link between satellites
- `find_edge()` - Find edge between two satellites
- `RouteConstraints` - Max latency, min reliability, forbidden nodes
- `OpticalRoute` - Path through topology with latency and reliability
- `PathFinder` - BFS-based route finding in topology
- `find_route()` - Find route respecting visibility and constraints
- `calculate_route_metrics()` - Compute latency and reliability for route
- `HandoffStrategy` - Pre-emptive handoff strategies
- `HandoffTrigger` - Triggers for handoff (degradation prediction, geometry change, SLA breach)
- `HandoffDecision` - Decision with trigger, strategy, and reason

### OISL Control Plane (`oisl/`)

#### Mission Module (`oisl/mission/`)
- `MissionIntent` - Declarative mission specification with objectives, constraints, SLAs
- `ObjectiveType` - Data relay, observation, custody transfer, downlink
- `Objective` - Specific mission objective with target and priority
- `IntentConstraints` - Max latency, min reliability, forbidden regions, max cost
- `ServiceLevelAgreement` - Latency, reliability, availability targets
- `TaskingPlan` - Compiled imperative plan from declarative intent
- `SatelliteTask` - Specific task for a satellite
- `LinkReservation` - Reserved link for task execution
- `PlanExplanation` - Routing, resource allocation, and tradeoff explanations
- `CompilationError` - Errors during intent compilation
- `ValidationWarning` - Warnings during plan validation
- `IntentCompiler` (trait) - Compile intents into tasking plans
- `DefaultIntentCompiler` - Default implementation with objective-specific compilation
- `compile()` - Compile mission intent into tasking plan
- `compile_data_relay()` - Compile data relay objective
- `compile_observation()` - Compile observation objective
- `compile_custody()` - Compile custody transfer objective
- `compile_downlink()` - Compile downlink objective
- `TaskingScheduler` - Schedule tasks with dependency and conflict management
- `schedule_tasks()` - Schedule tasks with timing calculation
- `detect_conflicts()` - Detect terminal contention and other conflicts
- `resolve_conflicts()` - Resolve conflicts with strategies
- `ConstellationState` - State of all satellites and terminals with bi-temporal versioning
- `resolve_asset()` - Resolve asset ID to satellite
- `find_optimal_ground_station()` - Find best ground station for downlink
- `calculate_geometry_score()` - Calculate link quality score
- `update_satellite_state()` - Update satellite position and status
- `update_terminal_state()` - Update terminal status
- `PlanValidator` - Validate tasking plans
- `validate_plan()` - Validate regulatory, SLA, resource, topology, timing compliance
- `generate_validation_report()` - Generate validation warnings and report

#### Topology Module (`oisl/topology/`)
- `TopologyForecast` - Time-indexed graph forecast with snapshots
- `GraphSnapshot` - Graph state at specific timestamp
- `NodeState` - Node position, velocity, terminals
- `NodeType` - Satellite or ground station
- `Position3D` - 3D position in ECI coordinates
- `Velocity3D` - 3D velocity
- `PotentialEdge` - Geometrically feasible link with visibility window
- `ActiveLink` - Currently established link with phase and metrics
- `TopologyInterpolation` - Linear, spline, or nearest interpolation
- `LinkObservation` - Observation for forecast refinement
- `RefinementReport` - Report after observation integration
- `TopologyForecaster` (trait) - Generate and refine topology forecasts
- `forecast()` - Generate topology forecast
- `refine()` - Refine forecast with new observations
- `query_at()` - Query topology at specific timestamp
- `query_window()` - Query topology over time window
- `get_at()` - Get snapshot at timestamp with interpolation
- `add_snapshot()` - Add snapshot to forecast
- `horizon_duration()` - Get forecast horizon duration
- `SpatiotemporalRouter` - Route through time-varying graph
- `Route` - Path through spatiotemporal graph with hops and cost
- `RoutedHop` - Single hop with edge, use window, and bandwidth reservation
- `CostScore` - Numerical cost score for routes
- `CostModel` - Weights for latency, capacity, reliability, power, PAT overhead
- `compute_route()` - Compute optimal route with Dijkstra on spatiotemporal graph
- `build_route()` - Build route from state
- `calculate_hop_cost()` - Calculate cost for single hop
- `satisfies_constraints()` - Check if route satisfies constraints
- `LinkPhase` - Idle, PatScheduled, Acquiring, Tracking, Communicating, Degrading, Lost
- `PeerHandshake` - Handshake during acquisition
- `FecState` - FEC state (Disabled, Enabled, Degraded)
- `LinkMetrics` - BER, SNR, latency, throughput, power, temperature
- `PhaseTransition` - Transition with bi-temporal recording
- `PhaseTransitionTrigger` - What caused the transition
- `ActiveLink` - Link with phase history and metrics
- `transition_to()` - Transition to new phase with bi-temporal recording
- `update_metrics()` - Update link metrics
- `is_active()` - Check if link is tracking or communicating
- `is_failed()` - Check if link is in failure state

#### Resource Module (`oisl/resource/`)
- `SatelliteNode` - Distributed system node with bounded resources
- `TerminalCapability` - Terminal capabilities for resource allocation
- `RfCapability` - RF terminal capabilities
- `SensorCapability` - Sensor capabilities
- `ComputeResources` - CPU, memory, GPU availability
- `StorageResources` - Storage capacity and usage
- `PowerBudget` - Power allocation and battery state
- `ThermalState` - Temperature and thermal margin
- `HealthMetrics` - Overall and component health
- `DegradationForecast` - Predicted component degradation
- `can_satisfy()` - Check if resource claim can be satisfied
- `allocate()` - Allocate resources for a claim
- `release()` - Release allocated resources
- `available_bandwidth()` - Calculate available bandwidth
- `add_pending_task()` - Add task to pending queue
- `pop_pending_task()` - Get next pending task
- `ResourceClaim` - Claim for optical terminal, bandwidth, power, storage, compute
- `ResourceAllocation` - Allocated resources with task, tenant, window, priority
- `new()` - Create new resource allocation
- `is_valid()` - Check if allocation is currently valid
- `can_preempt()` - Check if allocation can be preempted
- `ResourceAllocator` - Multi-tenant resource allocation with indexing
- `set_tenant_quota()` - Set quota for tenant
- `allocate()` - Attempt to allocate resources
- `release()` - Release allocation
- `preempt()` - Preempt allocation for higher priority task
- `check_conflicts()` - O(k) conflict check via terminal index
- `tenant_usage()` - O(k) tenant usage via tenant index
- `get_tenant_allocations()` - Get allocations for tenant
- `get_all_allocations()` - Get all allocations
- `TenantQuota` - Max power, bandwidth, storage, allocations per tenant
- `AllocationError` - Errors during allocation
- `SatelliteScheduler` (trait) - Kubernetes-like scheduling across satellites
- `allocate()` - Allocate resources for claim
- `preempt()` - Preempt allocation
- `rebalance()` - Rebalance allocations across satellites
- `forecast_availability()` - Forecast availability over time window
- `DefaultSatelliteScheduler` - Default scheduler implementation
- `add_satellite()` - Add satellite to scheduler
- `find_best_satellite()` - Find satellite with most available resources
- `RebalanceReport` - Report of rebalancing operations
- `AvailabilityForecast` - Forecast of satellite availability
- `SatelliteAvailability` - Per-satellite availability metrics
- `ScheduleError` - Errors during scheduling

#### Physical Module (`oisl/physical/`)
- `OctConfiguration` - SDA OCT Standard configuration
- `Modulation` - OOK-NRZ, Manchester, Manchester BM12, Manchester BM16
- `FecConfiguration` - FEC configuration with code and rate
- `FecCode` - 5G NR LDPC variants
- `LdpcVariant` - BaseGraph1 or BaseGraph2
- `CodeRate` - R1/2, R2/3, R3/4, R5/6
- `ArqConfiguration` - ARQ configuration with retransmissions and timeout
- `LinkType` - S2S (Space-to-Space) or S2T (Space-to-Terrestrial)
- `OctStandardVersion` - V3.0, V3.1, V3.2, V4.0.0
- `BaudRate` - Baud rate wrapper
- `default_s2s()` - Default S2S configuration (high SNR, ARQ off)
- `default_s2t()` - Default S2T configuration (low SNR, ARQ on)
- `OpticalTerminal` (trait) - Vendor abstraction over SDA OCT
  - `vendor()` - Get vendor
  - `model()` - Get model
  - `serial()` - Get serial number
  - `oct_standard_versions()` - Get supported OCT versions
  - `capabilities()` - Get terminal capabilities
  - `configure()` - Configure terminal
  - `calibrate()` - Calibrate terminal
  - `schedule_acquisition()` - Schedule PAT acquisition
  - `begin_pat()` - Begin PAT
  - `cancel_pat()` - Cancel PAT
  - `data_path()` - Get data path endpoint
  - `telemetry_stream()` - Get telemetry stream
  - `health_check()` - Perform health check
  - `reset()` - Reset terminal
  - `safe_mode()` - Enter safe mode
  - `get_status()` - Get terminal status
- `TerminalCapability` - Max data rate, pointing accuracy, standards, beam divergence, max range
- `CalibrationReport` - Calibration results with accuracy and next calibration
- `AcquisitionSchedule` - PAT acquisition schedule
- `PatHandle` - Handle for ongoing PAT
- `EthernetEndpoint` - Data path endpoint with IP, port, VLAN
- `TelemetryStream` - Stream of telemetry frames
- `TelemetryFrame` - Single telemetry frame
- `HealthReport` - Health report with component health and warnings
- `TerminalStatus` - Terminal operational status
- `TerminalError` - Terminal operation errors
- `Vendor` - Vendor enum (Mynaric, Tesat, Skyloom, Caci, Other)
- `SerialNumber` - Serial number wrapper
- `CondorMk3` - Mynaric CONDOR Mk3 implementation
- `Scot80` - Tesat SCOT80 implementation

#### PAT Coordination Module (`oisl/pat/`)
- `PatCoordinator` - Synchronized acquisition scheduling
- `schedule_acquisition()` - Schedule acquisition for terminal pair
- `get_pending_acquisitions()` - Get pending acquisitions
- `get_ready_acquisitions()` - Get acquisitions ready to execute (within 1 second)
- `cancel_acquisition()` - Cancel acquisition
- `clock_confidence()` - Get clock confidence
- `sync_clock()` - Sync clock to reference
- `event_log()` - Get event log
- `ScheduledAcquisition` - Scheduled acquisition with pre-computed parameters
- `PointingVector` - Pointing vector (azimuth, elevation, range)
- `AcquisitionSequence` - Sequence of acquisition steps
- `AcquisitionStep` - Single acquisition step with duration and action
- `AcquisitionAction` - Coarse point, fine point, beacon transmit, beacon receive, lock
- `FallbackAction` - Retry with wider beam, retry at later time, use alternative terminal, abort
- `PatEvent` - PAT event with bi-temporal recording
- `PatEventType` - Scheduled, Started, Acquiring, Tracking, Lost, Cancelled, Failed
- `OperatorAction` - Operator action for audit trail
- `SystemDecision` - System decision for audit trail
- `PatError` - PAT coordination errors
- `PrecisionClock` (trait) - Precision clock trait
  - `now()` - Get current precision timestamp
  - `confidence()` - Get clock confidence
  - `sync_to()` - Sync to time reference
  - `is_synchronized()` - Check if clock is synchronized
- `PrecisionTimestamp` - Precision timestamp with nanosecond precision
- `new()` - Create precision timestamp
- `from_datetime()` - Convert from DateTime
- `to_datetime()` - Convert to DateTime
- `now()` - Get current precision timestamp
- `duration_since()` - Calculate duration since another timestamp
- `ClockConfidence` - Clock confidence in nanoseconds
- `new()` - Create clock confidence
- `is_high_precision()` - Check if high precision (< 1 microsecond)
- `is_degraded()` - Check if degraded (> 100 microseconds)
- `gps_disciplined()` - GPS-disciplined oscillator accuracy (50 ns)
- `standard_oscillator()` - Standard oscillator accuracy (10 ms)
- `DefaultPrecisionClock` - Default precision clock implementation
- `with_confidence()` - Create clock with specified confidence
- `SyncReport` - Report of clock synchronization
- `ClockError` - Clock synchronization errors

#### Federation Module (`oisl/federation/`)
- `FederationPlane` - Cross-operator OISL coordination
- `add_peer()` - Add federation peer
- `establish_link()` - Establish cross-operator link
- `get_peer()` - Get peer by operator ID
- `get_cross_operator_links()` - Get all cross-operator links
- `FederationPeer` - Federation peer with trust score and policy
- `TrustScore` - Empirical trust score (0.0 to 1.0)
- `new()` - Create trust score
- `is_high()` - Check if high trust (>= 0.8)
- `is_low()` - Check if low trust (< 0.5)
- `FederationPolicy` - Federation policy for peer
- `allows_cross_operator_links()` - Check if policy allows cross-operator links
- `CrossOperatorLink` - Cross-operator link with revenue sharing and provenance
- `RevenueAgreement` - Revenue sharing agreement
- `ProvenanceChain` - Data provenance chain for cross-operator data
- `new()` - Create new provenance chain
- `add_hop()` - Add hop to provenance chain
- `ProvenanceHop` - Single hop in provenance chain
- `AttestationEngine` - Cryptographic attestation for peers
- `attest_peer()` - Attest a peer
- `verify_attestation()` - Verify attestation
- `AttestationRecord` - Attestation record
- `is_valid()` - Check if attestation is valid
- `FederationError` - Federation operation errors

### Core (`core/`)
- Central types: `SatelliteId`, `StationId`, `PassId`, `CustomerId`, `Frequency`
- Error handling with `thiserror`

### RF Layer (`rf-layer/`)
- `SdrHandle` abstraction for hardware
- `Sample` with bi-temporal timestamps (event time + reception time)
- `SampleRingBuffer` for no-alloc sample storage
- `DopplerPredictor` with SGP4 integration and UKF refinement
- `DemodState` with snapshotting for failover
- `ShadowTracker` for managing primary/shadow SDRs

### Tracking (`tracking/`)
- `TleData` and `TleSet` for Two-Line Element management
- `Propagator` using SGP4 with interpolation cache
- `UkfRefiner` for orbital state refinement using Doppler observations

### Hardware (`hardware/`)
- `PassShard` using `bumpalo` for per-pass memory isolation
- `HardwarePool` for device allocation
- `EmergencyShardPool` for pre-allocated failover resources
- `TenantShard` with cryptographic isolation

### Scheduler (`scheduler/`)
- `DominantResourceFairness` for fair resource allocation
- `ReputationTracker` for tenant reputation weighting
- `ScheduleOptimizer` using simulated annealing
- DRF prevents gaming: submitting more requests doesn't shift dominant resource

### Bi-temporal Logging (`bitemporal/`)
- `BiTemporal<T>` with event time and reception time
- `BitemporalLog` with chain integrity verification
- `ProvenanceChain` for complete data history
- `LogSnapshot` for time-travel queries

### Regulatory (`regulatory/`)
- `Band` trait for type-level frequency enforcement
- `License<B>` typed to specific bands
- `transmit<B>()` requires license for band B
- Compile-time enforcement: code doesn't compile without license

### Federation (`federation/`)
- `FederationPeer` with trust scores
- `Attestation` with cryptographic signatures
- `ChallengePass` for cross-verification
- Trust scoring based on verification history

### Agent (`agent/`)
- `ObservationTool` for read-only system state access
- `AnomalySynthesizer` for weak signal aggregation
- `ActionProposal` with operator approval workflow
- `AgentContext` for natural language queries

### Deployment (`deployment/`)
- `Supervisor` for pass subprocess management
- `HandoffManager` for zero-downtime deployment
- `VersionedBinary` with schema compatibility checking
- Pass-isolated supervision enables deployment without dropping passes

### Caching (`caching/`)
- `PropagationCache` with interpolation
- `TleCache` with atomic batch refresh
- `ScheduleFragmentCache` for incremental optimization
- Generic LRU and time-based caches

### Batching (`batching/`)
- `TleBatcher` for atomic constellation updates
- `DemodulatorBatcher` for Float Protocols output
- `LogBatcher` for efficient log storage

### Snapshotting (`snapshotting/`)
- `ScheduleSnapshot` for schedule state
- `DemodulatorSnapshot` for failover recovery
- `FederationSnapshot` for cross-station exchange
- Central `SnapshotManager` for coordination

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test
```

Run tests with simulated SDR backend:

```bash
cargo test --features sdr-sim
```

## Running

```bash
cargo run --bin ground-core
```

## Dependencies

Key dependencies:
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `chrono` - Time handling with bi-temporal support
- `thiserror` - Error handling
- `tracing` - Structured logging
- `sgp4` - Orbital propagation
- `nalgebra` - Linear algebra for geometry computations
- `rustfft` - FFT for signal processing
- `bumpalo` - No-heap allocations
- `ed25519-dalek` - Cryptographic signatures
- `sha2` - Hashing
- `uuid` - UUID generation for identifiers
- `async-trait` - Async trait support
- `futures` - Async stream utilities
- `tokio-stream` - Tokio stream adapters
- `reqwest` - HTTP client for vendor APIs
- `bitemporal` - Bi-temporal data structures

## License

Apache 2.0

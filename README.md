# Ground Core

GroundCore is the open-source alternative to proprietary satellite uplink and orchestration platforms. It is Apache 2.0, self-hosted, and the only open-source system that orchestrates both RF and laser optical communications in a single unified Rust binary.

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

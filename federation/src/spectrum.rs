//! Automated bilateral spectrum coordination between trusted federation peers
//!
//! Spectrum licenses look clean on paper. The operational reality is that
//! adjacent operators interfere with each other constantly — sidelobes and
//! intermodulation products land in neighbouring bands even with clean licenses.
//! Your S-band downlink at 2.2 GHz has neighbors at 2.19 and 2.21 GHz whose
//! sidelobes land in your band. Filing complaints to the ITU takes months.
//!
//! Real operators resolve this through informal bilateral coordination: sharing
//! pass schedules privately, swapping time slots, and resolving conflicts
//! directly. This entire layer of operational coordination is invisible in
//! any technical documentation.
//!
//! The federation layer is actually doing something operationally meaningful
//! here. The cryptographic attestation already establishes exactly the trust
//! relationship needed for bilateral coordination: a peer you trust enough to
//! cross-verify satellite passes is the same peer you can trust with your
//! pass schedule for interference avoidance.
//!
//! This module implements that coordination. "Automated bilateral spectrum
//! coordination between trusted peers" is a real operational need that doesn't
//! appear in any whitepaper — but it's what the operators are doing by phone.

use crate::peer::{FederationPeer, PeerId};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A frequency band specification for interference geometry calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationBand {
    /// Center frequency (Hz)
    pub center_hz: f64,
    /// Occupied bandwidth (Hz)
    pub bandwidth_hz: f64,
    /// Polarization (e.g., "RHCP", "LHCP", "linear-H", "linear-V")
    pub polarization: String,
}

impl CoordinationBand {
    pub fn s_band_downlink() -> Self {
        Self {
            center_hz: 2.2e9,
            bandwidth_hz: 20e6,
            polarization: "RHCP".to_string(),
        }
    }

    pub fn x_band_downlink() -> Self {
        Self {
            center_hz: 8.4e9,
            bandwidth_hz: 100e6,
            polarization: "RHCP".to_string(),
        }
    }

    pub fn s_band_uplink() -> Self {
        Self {
            center_hz: 2.025e9,
            bandwidth_hz: 10e6,
            polarization: "LHCP".to_string(),
        }
    }

    /// Frequency overlap with another band (Hz). Returns 0 if no overlap.
    pub fn overlap_hz(&self, other: &CoordinationBand) -> f64 {
        let self_lo = self.center_hz - self.bandwidth_hz / 2.0;
        let self_hi = self.center_hz + self.bandwidth_hz / 2.0;
        let other_lo = other.center_hz - other.bandwidth_hz / 2.0;
        let other_hi = other.center_hz + other.bandwidth_hz / 2.0;
        (self_hi.min(other_hi) - self_lo.max(other_lo)).max(0.0)
    }
}

/// A pass schedule entry shared with a federation peer for interference coordination.
///
/// Deliberately minimal: we share only what's needed to assess interference geometry.
/// No payload data, mission parameters, or customer information is included.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassScheduleEntry {
    pub entry_id: Uuid,
    /// Our station ID
    pub station_id: String,
    /// Satellite NORAD ID (for geometry context only)
    pub satellite_norad_id: u64,
    /// Frequency band in use during this pass
    pub band: CoordinationBand,
    /// Pass window start
    pub window_start: DateTime<Utc>,
    /// Pass window end
    pub window_end: DateTime<Utc>,
    /// Maximum transmit EIRP (dBW) — relevant for uplink interference estimation
    pub max_eirp_dbw: Option<f64>,
}

impl PassScheduleEntry {
    pub fn duration_s(&self) -> i64 {
        (self.window_end - self.window_start).num_seconds()
    }

    pub fn time_overlap(&self, other: &PassScheduleEntry) -> bool {
        self.window_start < other.window_end && other.window_start < self.window_end
    }

    pub fn time_overlap_s(&self, other: &PassScheduleEntry) -> i64 {
        if !self.time_overlap(other) {
            return 0;
        }
        let start = self.window_start.max(other.window_start);
        let end = self.window_end.min(other.window_end);
        (end - start).num_seconds()
    }
}

/// A detected spectrum conflict between our pass and a peer's pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumConflict {
    pub our_entry: PassScheduleEntry,
    pub peer_entry: PassScheduleEntry,
    pub peer_id: PeerId,
    /// Frequency overlap (Hz)
    pub frequency_overlap_hz: f64,
    /// Time overlap (seconds)
    pub time_overlap_s: i64,
    /// Estimated interference severity (0..1; higher = worse)
    pub severity: f64,
    pub detected_at: DateTime<Utc>,
}

impl SpectrumConflict {
    /// Whether this conflict is severe enough to require a scheduling adjustment.
    pub fn requires_coordination(&self) -> bool {
        self.severity > 0.3 || (self.frequency_overlap_hz > 0.0 && self.time_overlap_s > 30)
    }
}

/// A coordination request: we share our upcoming schedule with a peer and ask
/// them to flag any conflicts from their side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationRequest {
    pub request_id: Uuid,
    pub from_station: String,
    pub to_station: PeerId,
    pub our_entries: Vec<PassScheduleEntry>,
    pub look_ahead_hours: u32,
    pub created_at: DateTime<Utc>,
}

impl CoordinationRequest {
    pub fn new(from_station: String, to_station: PeerId, entries: Vec<PassScheduleEntry>) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            from_station,
            to_station,
            our_entries: entries,
            look_ahead_hours: 24,
            created_at: Utc::now(),
        }
    }
}

/// A coordination response: the peer flags conflicts on their side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationResponse {
    pub request_id: Uuid,
    pub from_station: PeerId,
    /// Passes the peer is running that conflict with our shared schedule
    pub their_conflicting_entries: Vec<PassScheduleEntry>,
    /// Full conflict detail list
    pub conflicts: Vec<SpectrumConflict>,
    /// Whether the peer accepted the coordination relationship
    pub accepted: bool,
    pub responded_at: DateTime<Utc>,
}

/// Per-peer bilateral coordination state.
#[derive(Debug)]
pub struct PeerCoordinationState {
    pub peer_id: PeerId,
    /// Most recent schedule snapshot shared by this peer
    pub their_schedule: Vec<PassScheduleEntry>,
    /// Active conflicts pending resolution
    pub active_conflicts: Vec<SpectrumConflict>,
    /// When we last exchanged schedules
    pub last_exchange: Option<DateTime<Utc>>,
    /// Count of successfully resolved coordination rounds
    pub successful_coordinations: u64,
}

impl PeerCoordinationState {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            their_schedule: Vec::new(),
            active_conflicts: Vec::new(),
            last_exchange: None,
            successful_coordinations: 0,
        }
    }

    pub fn needs_refresh(&self, interval: Duration) -> bool {
        match self.last_exchange {
            None => true,
            Some(last) => (Utc::now() - last) > interval,
        }
    }

    fn detect_conflicts(
        &self,
        our_entries: &[PassScheduleEntry],
        their_entries: &[PassScheduleEntry],
        peer_id: &PeerId,
    ) -> Vec<SpectrumConflict> {
        let mut conflicts = Vec::new();

        for ours in our_entries {
            for theirs in their_entries {
                if !ours.time_overlap(theirs) {
                    continue;
                }
                let freq_overlap = ours.band.overlap_hz(&theirs.band);
                if freq_overlap == 0.0 {
                    continue;
                }

                let time_overlap = ours.time_overlap_s(theirs);
                // Severity: fraction of bandwidth overlapping × fraction of pass overlapping
                let freq_fraction = freq_overlap / ours.band.bandwidth_hz;
                let time_fraction = time_overlap as f64 / ours.duration_s().max(1) as f64;
                let severity = (freq_fraction * time_fraction).min(1.0);

                conflicts.push(SpectrumConflict {
                    our_entry: ours.clone(),
                    peer_entry: theirs.clone(),
                    peer_id: peer_id.clone(),
                    frequency_overlap_hz: freq_overlap,
                    time_overlap_s: time_overlap,
                    severity,
                    detected_at: Utc::now(),
                });
            }
        }

        conflicts
    }
}

/// Spectrum coordinator — manages automated bilateral coordination with trusted peers.
///
/// Usage:
/// 1. Call `register_peer` for each trusted federation peer (trust_score ≥ 0.8).
/// 2. After each schedule optimization, call `update_our_schedule`.
/// 3. Call `check_our_schedule` to detect conflicts before committing passes.
/// 4. When a peer sends a `CoordinationRequest`, call `receive_request` to
///    generate a response and update the local conflict log.
/// 5. Periodically call `peers_needing_refresh` and send fresh `CoordinationRequest`s.
pub struct SpectrumCoordinator {
    our_station_id: String,
    peer_state: HashMap<PeerId, PeerCoordinationState>,
    our_schedule: Vec<PassScheduleEntry>,
    exchange_interval: Duration,
}

impl SpectrumCoordinator {
    pub fn new(station_id: String) -> Self {
        Self {
            our_station_id: station_id,
            peer_state: HashMap::new(),
            our_schedule: Vec::new(),
            exchange_interval: Duration::hours(2),
        }
    }

    /// Register a federation peer for bilateral coordination.
    /// Only peers with trust_score ≥ 0.8 (Trusted status) are accepted.
    pub fn register_peer(&mut self, peer: &FederationPeer) {
        if peer.is_trusted() {
            self.peer_state
                .entry(peer.station_id.clone())
                .or_insert_with(|| PeerCoordinationState::new(peer.station_id.clone()));
            tracing::info!(
                "Registered {} for bilateral spectrum coordination",
                peer.station_id
            );
        } else {
            tracing::warn!(
                "Skipping spectrum coordination for untrusted peer {} (trust_score={:.2})",
                peer.station_id,
                peer.trust_score
            );
        }
    }

    /// Update our planned schedule. Called by the scheduler after each optimization.
    pub fn update_our_schedule(&mut self, entries: Vec<PassScheduleEntry>) {
        self.our_schedule = entries;
    }

    /// Check our schedule against all known peer schedules. Returns detected conflicts.
    ///
    /// Call this after each schedule update. Conflicts with `requires_coordination()`
    /// true should trigger a `CoordinationRequest` to the affected peer.
    pub fn check_our_schedule(&self) -> Vec<SpectrumConflict> {
        let mut all_conflicts = Vec::new();
        for (peer_id, state) in &self.peer_state {
            let probe = PeerCoordinationState::new(peer_id.clone());
            let conflicts =
                probe.detect_conflicts(&self.our_schedule, &state.their_schedule, peer_id);
            all_conflicts.extend(conflicts);
        }

        if !all_conflicts.is_empty() {
            tracing::warn!(
                "{} spectrum conflicts detected across {} registered peers",
                all_conflicts.len(),
                self.peer_state.len()
            );
        }

        all_conflicts
    }

    /// Process an incoming coordination request from a trusted peer.
    ///
    /// Updates our local record of their schedule, detects conflicts from our side,
    /// and returns a response for transmission back to the peer.
    pub fn receive_request(
        &mut self,
        request: CoordinationRequest,
        peer: &FederationPeer,
    ) -> CoordinationResponse {
        if !peer.is_trusted() {
            tracing::warn!(
                "Rejecting coordination request from untrusted peer {}",
                peer.station_id
            );
            return CoordinationResponse {
                request_id: request.request_id,
                from_station: self.our_station_id.clone(),
                their_conflicting_entries: vec![],
                conflicts: vec![],
                accepted: false,
                responded_at: Utc::now(),
            };
        }

        if let Some(state) = self.peer_state.get_mut(&peer.station_id) {
            state.their_schedule = request.our_entries.clone();
            state.last_exchange = Some(Utc::now());
        }

        let probe = PeerCoordinationState::new(peer.station_id.clone());
        let conflicts =
            probe.detect_conflicts(&self.our_schedule, &request.our_entries, &peer.station_id);

        if !conflicts.is_empty() {
            tracing::warn!(
                "{} conflicts detected responding to coordination request from {}",
                conflicts.len(),
                peer.station_id
            );
            for c in &conflicts {
                tracing::warn!(
                    "  freq_overlap={:.1} MHz, time_overlap={}s, severity={:.2}",
                    c.frequency_overlap_hz / 1e6,
                    c.time_overlap_s,
                    c.severity
                );
            }
        }

        let our_conflicting: Vec<PassScheduleEntry> =
            conflicts.iter().map(|c| c.our_entry.clone()).collect();

        CoordinationResponse {
            request_id: request.request_id,
            from_station: self.our_station_id.clone(),
            their_conflicting_entries: our_conflicting,
            conflicts,
            accepted: true,
            responded_at: Utc::now(),
        }
    }

    /// Process an incoming coordination response from a peer.
    pub fn receive_response(&mut self, response: CoordinationResponse) {
        if let Some(state) = self.peer_state.get_mut(&response.from_station) {
            if !response.conflicts.is_empty() {
                tracing::warn!(
                    "Peer {} reports {} conflicts with our schedule",
                    response.from_station,
                    response.conflicts.len()
                );
            }
            state.active_conflicts = response.conflicts;
            state.last_exchange = Some(Utc::now());
            if response.accepted {
                state.successful_coordinations += 1;
            }
        }
    }

    /// Build a coordination request to send to a specific peer.
    pub fn build_request(&self, peer_id: &PeerId) -> Option<CoordinationRequest> {
        if self.peer_state.contains_key(peer_id) {
            Some(CoordinationRequest::new(
                self.our_station_id.clone(),
                peer_id.clone(),
                self.our_schedule.clone(),
            ))
        } else {
            None
        }
    }

    /// List peers whose coordination data needs refreshing.
    pub fn peers_needing_refresh(&self) -> Vec<&PeerId> {
        self.peer_state
            .iter()
            .filter(|(_, state)| state.needs_refresh(self.exchange_interval))
            .map(|(id, _)| id)
            .collect()
    }

    /// All active conflicts across all peers.
    pub fn active_conflicts(&self) -> Vec<&SpectrumConflict> {
        self.peer_state
            .values()
            .flat_map(|s| s.active_conflicts.iter())
            .collect()
    }

    /// Count of registered peers.
    pub fn peer_count(&self) -> usize {
        self.peer_state.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(center_hz: f64, start_offset_min: i64, duration_min: i64) -> PassScheduleEntry {
        let now = Utc::now();
        PassScheduleEntry {
            entry_id: Uuid::new_v4(),
            station_id: "station-a".to_string(),
            satellite_norad_id: 12345,
            band: CoordinationBand {
                center_hz,
                bandwidth_hz: 20e6,
                polarization: "RHCP".to_string(),
            },
            window_start: now + Duration::minutes(start_offset_min),
            window_end: now + Duration::minutes(start_offset_min + duration_min),
            max_eirp_dbw: Some(43.0),
        }
    }

    #[test]
    fn test_no_conflict_different_bands() {
        let ours = vec![make_entry(2.2e9, 0, 10)];
        let theirs = vec![make_entry(8.4e9, 0, 10)]; // X-band vs S-band — no overlap
        let state = PeerCoordinationState::new("peer-1".to_string());
        let conflicts = state.detect_conflicts(&ours, &theirs, &"peer-1".to_string());
        assert!(conflicts.is_empty(), "Different bands should not conflict");
    }

    #[test]
    fn test_conflict_same_band_overlapping_time() {
        let ours = vec![make_entry(2.2e9, 0, 10)];
        let theirs = vec![make_entry(2.2e9, 5, 10)]; // 5-minute overlap
        let state = PeerCoordinationState::new("peer-1".to_string());
        let conflicts = state.detect_conflicts(&ours, &theirs, &"peer-1".to_string());
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].frequency_overlap_hz > 0.0);
        assert!(conflicts[0].time_overlap_s > 0);
    }

    #[test]
    fn test_no_conflict_sequential_time() {
        let ours = vec![make_entry(2.2e9, 0, 10)];
        let theirs = vec![make_entry(2.2e9, 10, 10)]; // back-to-back, no overlap
        let state = PeerCoordinationState::new("peer-1".to_string());
        let conflicts = state.detect_conflicts(&ours, &theirs, &"peer-1".to_string());
        assert!(
            conflicts.is_empty(),
            "Sequential passes should not conflict"
        );
    }

    #[test]
    fn test_partial_band_overlap() {
        // Our band: 2200 ± 10 MHz (2190–2210 MHz)
        // Their band: 2205 ± 10 MHz (2195–2215 MHz)
        // Overlap: 2195–2210 = 15 MHz
        let ours = vec![make_entry(2200e6, 0, 10)];
        let theirs = vec![make_entry(2205e6, 0, 10)];
        let state = PeerCoordinationState::new("peer-1".to_string());
        let conflicts = state.detect_conflicts(&ours, &theirs, &"peer-1".to_string());
        assert_eq!(conflicts.len(), 1);
        assert!(
            (conflicts[0].frequency_overlap_hz - 15e6).abs() < 1e3,
            "Expected ~15 MHz overlap, got {} Hz",
            conflicts[0].frequency_overlap_hz
        );
    }

    #[test]
    fn test_severity_proportional_to_overlap() {
        let full_overlap = {
            let ours = vec![make_entry(2.2e9, 0, 10)];
            let theirs = vec![make_entry(2.2e9, 0, 10)]; // full time + freq overlap
            let state = PeerCoordinationState::new("peer-1".to_string());
            state.detect_conflicts(&ours, &theirs, &"peer-1".to_string())
        };
        let partial_overlap = {
            let ours = vec![make_entry(2.2e9, 0, 10)];
            let theirs = vec![make_entry(2.2e9, 8, 10)]; // 2-minute time overlap
            let state = PeerCoordinationState::new("peer-1".to_string());
            state.detect_conflicts(&ours, &theirs, &"peer-1".to_string())
        };
        assert!(
            full_overlap[0].severity > partial_overlap[0].severity,
            "Full overlap should have higher severity than partial"
        );
    }
}

// PAT attestation
//
// PAT attestations provide cryptographic proof that acquisition and tracking
// occurred with specific parameters at specific times.

use crate::{BiTemporal, EventTime, ReceptionTime};
use crate::attestation::link_proof::{AttestationId, Hash, Signature};
use crate::pat::AcquisitionResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// PAT attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatAttestation {
    pub attestation_id: AttestationId,
    pub acquisition_id: Uuid,
    pub acquisition_proof: AcquisitionProof,
    pub tracking_proof: Option<TrackingProof>,
    pub hash: Hash,
    pub signature: Signature,
}

impl PatAttestation {
    pub fn new(acquisition_id: Uuid, acquisition_proof: AcquisitionProof) -> Self {
        Self {
            attestation_id: Uuid::new_v4(),
            acquisition_id,
            acquisition_proof,
            tracking_proof: None,
            hash: Hash::new("placeholder_hash".to_string()),
            signature: Signature::new("placeholder_signature".to_string()),
        }
    }

    pub fn with_tracking_proof(mut self, tracking_proof: TrackingProof) -> Self {
        self.tracking_proof = Some(tracking_proof);
        self
    }
}

/// Acquisition proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionProof {
    pub plan_id: Uuid,
    pub start_time: BiTemporal<DateTime<Utc>>,
    pub end_time: BiTemporal<DateTime<Utc>>,
    pub result: AcquisitionResult,
    pub pointing_vectors: (String, String), // JSON-serialized pointing vectors
    pub search_pattern_used: String,
}

impl AcquisitionProof {
    pub fn new(plan_id: Uuid, result: AcquisitionResult, search_pattern_used: String) -> Self {
        let now = Utc::now();
        Self {
            plan_id,
            start_time: BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now)),
            end_time: BiTemporal::new(now, EventTime::new(now), ReceptionTime::new(now)),
            result,
            pointing_vectors: ("{}".to_string(), "{}".to_string()),
            search_pattern_used,
        }
    }
}

/// Tracking proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingProof {
    pub tracking_id: Uuid,
    pub start_time: BiTemporal<DateTime<Utc>>,
    pub end_time: BiTemporal<DateTime<Utc>>,
    pub duration_seconds: f64,
    pub average_pointing_error_urad: f64,
    pub average_snr_db: f64,
    pub lock_confidence: f64,
}

impl TrackingProof {
    pub fn new(tracking_id: Uuid) -> Self {
        Self {
            tracking_id,
            start_time: BiTemporal::new(Utc::now(), EventTime::new(Utc::now()), ReceptionTime::new(Utc::now())),
            end_time: BiTemporal::new(Utc::now(), EventTime::new(Utc::now()), ReceptionTime::new(Utc::now())),
            duration_seconds: 0.0,
            average_pointing_error_urad: 0.0,
            average_snr_db: 0.0,
            lock_confidence: 0.0,
        }
    }

    pub fn with_metrics(
        mut self,
        duration_seconds: f64,
        average_pointing_error_urad: f64,
        average_snr_db: f64,
        lock_confidence: f64,
    ) -> Self {
        self.duration_seconds = duration_seconds;
        self.average_pointing_error_urad = average_pointing_error_urad;
        self.average_snr_db = average_snr_db;
        self.lock_confidence = lock_confidence;
        self
    }
}

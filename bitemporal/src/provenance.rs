//! Provenance chain and verification
//!
//! The provenance chain tracks the complete history of data from satellite
//! emission to final delivery, making the system provably auditable.

use crate::timestamp::{EventTime, ReceptionTime};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provenance chain for a single pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceChain {
    /// Pass ID
    pub pass_id: PassId,
    /// Satellite ID
    pub satellite_id: String,
    /// Ground station ID
    pub station_id: String,
    /// Chain links in chronological order
    pub links: Vec<ProvenanceLink>,
    /// Chain hash for integrity verification
    pub chain_hash: String,
}

/// Single link in the provenance chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceLink {
    /// Link type
    pub link_type: ProvenanceLinkType,
    /// Event time
    pub event_time: EventTime,
    /// Reception time
    pub reception_time: ReceptionTime,
    /// Link data
    pub data: serde_json::Value,
    /// Hash of this link
    pub hash: String,
    /// Previous link hash
    pub previous_hash: Option<String>,
    /// Signature (for federation links)
    pub signature: Option<String>,
}

/// Types of provenance links
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProvenanceLinkType {
    /// Satellite emission
    SatelliteEmission {
        position: (f64, f64, f64),
        frequency: u64,
    },
    /// RF reception
    RfReception {
        device_id: String,
        antenna_id: String,
        frequency: u64,
    },
    /// Demodulation
    Demodulation {
        algorithm: String,
        bits_decoded: u64,
    },
    /// Protocol translation (Float Protocols)
    ProtocolTranslation {
        protocol: String,
        events_generated: u64,
    },
    /// Data normalization (Mandala)
    DataNormalization {
        schema_version: String,
        records_normalized: u64,
    },
    /// Federation handoff
    FederationHandoff {
        from_station: String,
        to_station: String,
    },
    /// Customer delivery
    CustomerDelivery {
        customer_id: String,
        delivery_method: String,
    },
}

impl ProvenanceChain {
    /// Create a new provenance chain
    pub fn new(pass_id: PassId, satellite_id: String, station_id: String) -> Self {
        Self {
            pass_id,
            satellite_id,
            station_id,
            links: Vec::new(),
            chain_hash: String::new(),
        }
    }

    /// Add a link to the chain
    pub fn add_link(&mut self, link: ProvenanceLink) -> Result<()> {
        // Verify chain integrity
        if let Some(last_link) = self.links.last()
            && link.previous_hash.as_ref() != Some(&last_link.hash)
        {
            return Err(ground_core::GroundStationError::Database(
                "Provenance chain integrity broken".to_string(),
            ));
        }

        self.links.push(link);
        self.recompute_chain_hash();

        Ok(())
    }

    /// Recpute the chain hash
    fn recompute_chain_hash(&mut self) {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(self.pass_id.as_bytes());
        hasher.update(self.satellite_id.as_bytes());
        hasher.update(self.station_id.as_bytes());

        for link in &self.links {
            hasher.update(link.hash.as_bytes());
        }

        self.chain_hash = format!("{:x}", hasher.finalize());
    }

    /// Verify the integrity of the chain
    pub fn verify(&self) -> bool {
        let mut previous_hash: Option<String> = None;

        for link in &self.links {
            if link.previous_hash != previous_hash {
                return false;
            }
            previous_hash = Some(link.hash.clone());
        }

        // Verify chain hash
        let mut computed_chain = self.clone();
        computed_chain.recompute_chain_hash();
        computed_chain.chain_hash == self.chain_hash
    }

    /// Get the complete event timeline
    pub fn timeline(&self) -> Vec<(EventTime, ReceptionTime, ProvenanceLinkType)> {
        self.links
            .iter()
            .map(|link| (link.event_time, link.reception_time, link.link_type.clone()))
            .collect()
    }

    /// Compute total propagation delay from satellite to customer
    pub fn total_propagation_delay(&self) -> chrono::Duration {
        if self.links.is_empty() {
            return chrono::Duration::zero();
        }

        let first = self.links.first().unwrap();
        let last = self.links.last().unwrap();

        last.reception_time.as_datetime() - first.event_time.as_datetime()
    }
}

/// Provenance verifier for cross-station validation
pub struct ProvenanceVerifier {
    /// Known station public keys
    station_keys: HashMap<String, String>,
}

impl ProvenanceVerifier {
    pub fn new() -> Self {
        Self {
            station_keys: HashMap::new(),
        }
    }

    /// Add a station's public key
    pub fn add_station_key(&mut self, station_id: String, public_key: String) {
        self.station_keys.insert(station_id, public_key);
    }

    /// Verify a provenance chain from a federated station
    pub fn verify_federated_chain(&self, chain: &ProvenanceChain) -> Result<bool> {
        // Verify chain integrity
        if !chain.verify() {
            return Ok(false);
        }

        // Verify signatures on federation handoff links
        for link in &chain.links {
            if matches!(link.link_type, ProvenanceLinkType::FederationHandoff { .. })
                && let Some(signature) = &link.signature
            {
                // In production, verify the signature with the station's public key
                // For now, we just check that a signature exists
                if signature.is_empty() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Cross-verify two chains from different stations (for challenge passes)
    pub fn cross_verify_chains(
        &self,
        chain1: &ProvenanceChain,
        chain2: &ProvenanceChain,
    ) -> Result<CrossVerificationResult> {
        // Both chains must be valid
        if !chain1.verify() || !chain2.verify() {
            return Ok(CrossVerificationResult {
                match_rate: 0.0,
                bitemporal_consistency: false,
                details: "One or both chains invalid".to_string(),
            });
        }

        // Check if both chains are for the same satellite pass
        if chain1.satellite_id != chain2.satellite_id {
            return Ok(CrossVerificationResult {
                match_rate: 0.0,
                bitemporal_consistency: false,
                details: "Different satellites".to_string(),
            });
        }

        // Compare event times (should be nearly identical for the same satellite emission)
        let mut matched_links = 0;
        let total_links = chain1.links.len() + chain2.links.len();

        for link1 in &chain1.links {
            for link2 in &chain2.links {
                let time_diff = (link1.event_time.as_datetime() - link2.event_time.as_datetime())
                    .num_milliseconds()
                    .abs();

                // Event times should match within 100ms for the same emission
                if time_diff < 100 {
                    matched_links += 1;
                }
            }
        }

        let match_rate = if total_links > 0 {
            (2 * matched_links) as f64 / total_links as f64
        } else {
            0.0
        };

        // Check bi-temporal consistency
        let bitemporal_consistency = match_rate > 0.8;

        Ok(CrossVerificationResult {
            match_rate,
            bitemporal_consistency,
            details: format!("Matched {} of {} links", matched_links, total_links / 2),
        })
    }
}

impl Default for ProvenanceVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of cross-verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVerificationResult {
    /// Percentage of matching links (0.0 to 1.0)
    pub match_rate: f64,
    /// Whether bi-temporal timestamps are consistent
    pub bitemporal_consistency: bool,
    /// Detailed explanation
    pub details: String,
}

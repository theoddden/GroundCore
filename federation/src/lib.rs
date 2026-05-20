//! Federation layer with cryptographic attestation
//!
//! This implements Problem 4: Federation with adversarial peers.
//!
//! Key concepts:
//! - Treat federation peers like Byzantine nodes — verify empirically
//! - Periodic challenge passes for cross-verification
//! - Cryptographic signing of captured data
//! - Trust scoring based on verification history
//! - Bi-temporal logging for provable verification

pub mod attestation;
pub mod challenge;
pub mod peer;
pub mod spectrum;
pub mod verification;

pub use attestation::{Attestation, SignedData};
pub use peer::{FederationPeer, PeerId, PeerManager, VerificationResult};
pub use spectrum::{
    CoordinationBand, CoordinationRequest, CoordinationResponse, PassScheduleEntry,
    SpectrumConflict, SpectrumCoordinator,
};
pub use verification::{
    AttestationVerification, ChallengePass, ChallengeResult, ChallengeSchedule,
};

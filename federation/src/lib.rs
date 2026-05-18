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

pub mod peer;
pub mod attestation;
pub mod verification;
pub mod challenge;

pub use peer::{FederationPeer, PeerId, PeerManager, VerificationResult};
pub use attestation::{Attestation, SignedData};
pub use verification::{AttestationVerification, ChallengeSchedule, ChallengePass, ChallengeResult};

// Bi-temporal cryptographic attestation
//
// Attestation primitives provide cryptographic proof of events with bi-temporal
// timestamps (event time and system observation time). This enables audit trails
// and forensic recovery.

pub mod link_proof;
pub mod pat_proof;
pub mod identity;

pub use link_proof::{LinkAttestation, AttestationId, AttestableEvent, Hash, Signature};
pub use pat_proof::{PatAttestation, AcquisitionProof, TrackingProof};
pub use identity::{TerminalIdentity, IdentityCertificate};

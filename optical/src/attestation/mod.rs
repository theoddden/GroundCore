// Bi-temporal cryptographic attestation
//
// Attestation primitives provide cryptographic proof of events with bi-temporal
// timestamps (event time and system observation time). This enables audit trails
// and forensic recovery.

pub mod identity;
pub mod link_proof;
pub mod pat_proof;

pub use identity::{IdentityCertificate, TerminalIdentity};
pub use link_proof::{AttestableEvent, AttestationId, Hash, LinkAttestation, Signature};
pub use pat_proof::{AcquisitionProof, PatAttestation, TrackingProof};

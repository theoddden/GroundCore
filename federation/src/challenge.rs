//! Challenge pass management

use crate::peer::{PeerId, VerificationResult};
use crate::verification::{AttestationVerification, ChallengePass, ChallengeResult};
use chrono::{DateTime, Utc};
use ground_core::{PassId, Result};
use serde::{Deserialize, Serialize};

/// Challenge pass manager
pub struct ChallengePassManager {
    challenges: Vec<ChallengePass>,
}

impl ChallengePassManager {
    pub fn new() -> Self {
        Self {
            challenges: Vec::new(),
        }
    }

    /// Schedule a challenge pass
    pub fn schedule_challenge(
        &mut self,
        pass_id: PassId,
        satellite_id: String,
        scheduled_time: DateTime<Utc>,
        peers_to_verify: Vec<PeerId>,
    ) {
        let challenge = ChallengePass {
            pass_id,
            satellite_id,
            scheduled_time,
            peers_to_verify,
            completed: false,
            results: Vec::new(),
        };

        self.challenges.push(challenge);
    }

    /// Get pending challenges
    pub fn pending_challenges(&self) -> Vec<&ChallengePass> {
        self.challenges
            .iter()
            .filter(|c| !c.completed && c.scheduled_time > Utc::now())
            .collect()
    }

    /// Get challenges due now
    pub fn due_challenges(&mut self) -> Vec<&mut ChallengePass> {
        self.challenges
            .iter_mut()
            .filter(|c| !c.completed && c.scheduled_time <= Utc::now())
            .collect()
    }

    /// Complete a challenge with results
    pub fn complete_challenge(
        &mut self,
        pass_id: &PassId,
        results: Vec<ChallengeResult>,
    ) -> Result<()> {
        if let Some(challenge) = self.challenges.iter_mut().find(|c| &c.pass_id == pass_id) {
            challenge.results = results;
            challenge.completed = true;
            Ok(())
        } else {
            Err(ground_core::GroundStationError::Federation(format!(
                "Challenge {} not found",
                pass_id
            )))
        }
    }

    /// Get challenge results for a peer
    pub fn get_peer_results(&self, peer_id: &PeerId) -> Vec<&ChallengeResult> {
        self.challenges
            .iter()
            .flat_map(|c| c.results.iter())
            .filter(|r| &r.peer_id == peer_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{SigningKey, VerifyingKey, Signer};
    use federation::attestation::{Attestation, SignedData};
    use federation::peer::{FederationPeer, PeerId};

    #[test]
    fn test_peer_creation() {
        let peer_id = PeerId::new("peer1".to_string());
        let peer = FederationPeer::new(peer_id);
        assert_eq!(peer.id().as_str(), "peer1");
    }

    #[test]
    fn test_attestation_creation() {
        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        let data = b"test data for attestation";
        let signature = signing_key.sign(data);

        let signed_data = SignedData::new(data.to_vec(), signature.to_bytes().to_vec());

        assert_eq!(signed_data.data(), data);
        assert_eq!(signed_data.signature().len(), 64);
    }

    #[test]
    fn test_signature_verification() {
        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        let data = b"test data for attestation";
        let signature = signing_key.sign(data);

        let signed_data = SignedData::new(data.to_vec(), signature.to_bytes().to_vec());

        let verification_result = verifying_key.verify(data, &signature);
        assert!(verification_result.is_ok());
    }

    #[test]
    fn test_signature_verification_fails_with_wrong_key() {
        let mut csprng = rand::rngs::OsRng;
        let signing_key1 = SigningKey::generate(&mut csprng);
        let verifying_key2 = SigningKey::generate(&mut csprng).verifying_key();

        let data = b"test data for attestation";
        let signature = signing_key1.sign(data);

        // Try to verify with wrong public key
        let verification_result = verifying_key2.verify(data, &signature);
        assert!(verification_result.is_err());
    }

    #[test]
    fn test_attestation_serialization() {
        let data = b"test data".to_vec();
        let signature = vec![0u8; 64];

        let signed_data = SignedData::new(data.clone(), signature.clone());

        let serialized = serde_json::to_string(&signed_data).unwrap();
        let deserialized: SignedData = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.data(), &data);
        assert_eq!(deserialized.signature(), &signature);
    }
}

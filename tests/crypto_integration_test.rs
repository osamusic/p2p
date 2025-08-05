use libp2p::identity;
use p2p_sync::crypto::{SignedData, SignedSyncMessage, SyncOperation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestMessage {
    content: String,
    value: u64,
}

#[test]
fn test_sign_and_verify_message() {
    let keypair = identity::Keypair::generate_ed25519();
    let message = TestMessage {
        content: "Hello, P2P!".to_string(),
        value: 42,
    };

    // Sign the message
    let signed = SignedData::new(message.clone(), &keypair).unwrap();

    // Verify with correct keypair
    assert!(signed.verify(&keypair).unwrap());

    // Verify with public key
    let public_key = keypair.public();
    assert!(signed.verify_with_public_key(&public_key).unwrap());

    // Verify with wrong keypair should fail
    let wrong_keypair = identity::Keypair::generate_ed25519();
    assert!(!signed.verify(&wrong_keypair).unwrap());
}

#[test]
fn test_signed_sync_message() {
    let keypair = identity::Keypair::generate_ed25519();

    let sync_msg = SignedSyncMessage {
        key: "test_key".to_string(),
        value: Some("test_value".to_string()),
        timestamp: chrono::Utc::now(),
        operation: SyncOperation::Put,
    };

    // Sign the sync message
    let signed = SignedData::new(sync_msg.clone(), &keypair).unwrap();

    // Serialize and deserialize
    let serialized = serde_json::to_string(&signed).unwrap();
    let deserialized: SignedData<SignedSyncMessage> = serde_json::from_str(&serialized).unwrap();

    // Verify the deserialized message
    assert!(deserialized.verify(&keypair).unwrap());
    assert_eq!(deserialized.data.key, "test_key");
    assert_eq!(deserialized.data.value, Some("test_value".to_string()));
}

#[test]
fn test_tampered_signature_detection() {
    let keypair = identity::Keypair::generate_ed25519();
    let message = TestMessage {
        content: "Original".to_string(),
        value: 100,
    };

    let mut signed = SignedData::new(message, &keypair).unwrap();

    // Tamper with the signature
    signed.signature[0] ^= 0xFF;

    // Verification should fail
    assert!(!signed.verify(&keypair).unwrap());
}

#[test]
fn test_tampered_data_detection() {
    let keypair = identity::Keypair::generate_ed25519();
    let message = TestMessage {
        content: "Original".to_string(),
        value: 100,
    };

    let mut signed = SignedData::new(message, &keypair).unwrap();

    // Tamper with the data
    signed.data.value = 200;

    // Verification should fail
    assert!(!signed.verify(&keypair).unwrap());
}

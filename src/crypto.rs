use anyhow::Result;
use libp2p::identity::Keypair;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedData<T: Serialize> {
    pub data: T,
    pub signature: Vec<u8>,
    pub signer: String,
}

impl<T: Serialize> SignedData<T> {
    pub fn new(data: T, keypair: &Keypair) -> Result<Self> {
        let data_bytes = bincode::serialize(&data)?;
        let hash = Sha256::digest(&data_bytes);
        
        let signature = keypair.sign(&hash)?;
        let signer = keypair.public().to_peer_id().to_string();
        
        Ok(Self {
            data,
            signature,
            signer,
        })
    }
    
    pub fn verify(&self, keypair: &Keypair) -> Result<bool> {
        let data_bytes = bincode::serialize(&self.data)?;
        let hash = Sha256::digest(&data_bytes);
        
        let public_key = keypair.public();
        
        let expected_signer = public_key.to_peer_id().to_string();
        if self.signer != expected_signer {
            return Ok(false);
        }
        
        Ok(public_key.verify(&hash, &self.signature))
    }
    
    pub fn verify_with_public_key(&self, public_key: &libp2p::identity::PublicKey) -> Result<bool> {
        let data_bytes = bincode::serialize(&self.data)?;
        let hash = Sha256::digest(&data_bytes);
        
        let expected_signer = public_key.to_peer_id().to_string();
        if self.signer != expected_signer {
            return Ok(false);
        }
        
        Ok(public_key.verify(&hash, &self.signature))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedSyncMessage {
    pub key: String,
    pub value: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub operation: SyncOperation,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncOperation {
    Put,
    Delete,
}

impl From<crate::sync::SyncMessage> for SignedSyncMessage {
    fn from(msg: crate::sync::SyncMessage) -> Self {
        match msg {
            crate::sync::SyncMessage::Put { key, value, timestamp } => Self {
                key,
                value: Some(value),
                timestamp,
                operation: SyncOperation::Put,
            },
            crate::sync::SyncMessage::Delete { key, timestamp } => Self {
                key,
                value: None,
                timestamp,
                operation: SyncOperation::Delete,
            },
        }
    }
}

impl From<SignedSyncMessage> for crate::sync::SyncMessage {
    fn from(msg: SignedSyncMessage) -> Self {
        match msg.operation {
            SyncOperation::Put => crate::sync::SyncMessage::Put {
                key: msg.key,
                value: msg.value.unwrap_or_default(),
                timestamp: msg.timestamp,
            },
            SyncOperation::Delete => crate::sync::SyncMessage::Delete {
                key: msg.key,
                timestamp: msg.timestamp,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;
    
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        message: String,
        value: u32,
    }
    
    #[test]
    fn test_sign_and_verify() {
        let keypair = identity::Keypair::generate_ed25519();
        let data = TestData {
            message: "Hello, world!".to_string(),
            value: 42,
        };
        
        let signed = SignedData::new(data.clone(), &keypair).unwrap();
        assert!(signed.verify(&keypair).unwrap());
        
        // Verify with wrong keypair should fail
        let wrong_keypair = identity::Keypair::generate_ed25519();
        assert!(!signed.verify(&wrong_keypair).unwrap());
    }
    
    #[test]
    fn test_sign_and_verify_with_public_key() {
        let keypair = identity::Keypair::generate_ed25519();
        let public_key = keypair.public();
        
        let data = TestData {
            message: "Test".to_string(),
            value: 123,
        };
        
        let signed = SignedData::new(data, &keypair).unwrap();
        assert!(signed.verify_with_public_key(&public_key).unwrap());
    }
    
    #[test]
    fn test_tampered_data() {
        let keypair = identity::Keypair::generate_ed25519();
        let data = TestData {
            message: "Original".to_string(),
            value: 100,
        };
        
        let mut signed = SignedData::new(data, &keypair).unwrap();
        
        // Tamper with the data
        signed.data.value = 200;
        
        // Verification should fail
        assert!(!signed.verify(&keypair).unwrap());
    }
    
    #[test]
    fn test_sync_message_conversion() {
        use crate::sync::SyncMessage;
        use chrono::Utc;
        
        let put_msg = SyncMessage::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: Utc::now(),
        };
        
        let signed_msg: SignedSyncMessage = put_msg.clone().into();
        assert_eq!(signed_msg.key, "test_key");
        assert_eq!(signed_msg.value, Some("test_value".to_string()));
        assert!(matches!(signed_msg.operation, SyncOperation::Put));
        
        let converted_back: SyncMessage = signed_msg.into();
        match converted_back {
            SyncMessage::Put { key, value, .. } => {
                assert_eq!(key, "test_key");
                assert_eq!(value, "test_value");
            }
            _ => panic!("Expected Put message"),
        }
    }
}
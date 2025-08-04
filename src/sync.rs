use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    Put {
        key: String,
        value: String,
        timestamp: DateTime<Utc>,
    },
    Delete {
        key: String,
        timestamp: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_sync_message_put_serialization() {
        let msg = SyncMessage::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: Utc::now(),
        };

        // Test serialization and deserialization
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: SyncMessage = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            SyncMessage::Put { key, value, .. } => {
                assert_eq!(key, "test_key");
                assert_eq!(value, "test_value");
            }
            _ => panic!("Expected Put message"),
        }
    }

    #[test]
    fn test_sync_message_delete_serialization() {
        let msg = SyncMessage::Delete {
            key: "test_key".to_string(),
            timestamp: Utc::now(),
        };

        // Test serialization and deserialization
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: SyncMessage = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            SyncMessage::Delete { key, .. } => {
                assert_eq!(key, "test_key");
            }
            _ => panic!("Expected Delete message"),
        }
    }

    #[test]
    fn test_sync_message_bincode_serialization() {
        let msg = SyncMessage::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: Utc::now(),
        };

        // Test bincode serialization
        let encoded = bincode::serialize(&msg).unwrap();
        let decoded: SyncMessage = bincode::deserialize(&encoded).unwrap();

        match decoded {
            SyncMessage::Put { key, value, .. } => {
                assert_eq!(key, "test_key");
                assert_eq!(value, "test_value");
            }
            _ => panic!("Expected Put message"),
        }
    }

    #[test]
    fn test_sync_message_clone() {
        let original = SyncMessage::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: Utc::now(),
        };

        let cloned = original.clone();
        
        match (original, cloned) {
            (SyncMessage::Put { key: k1, value: v1, timestamp: t1 }, 
             SyncMessage::Put { key: k2, value: v2, timestamp: t2 }) => {
                assert_eq!(k1, k2);
                assert_eq!(v1, v2);
                assert_eq!(t1, t2);
            }
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_sync_message_debug_format() {
        let msg = SyncMessage::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            timestamp: Utc::now(),
        };

        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("Put"));
        assert!(debug_str.contains("test_key"));
        assert!(debug_str.contains("test_value"));
    }
}

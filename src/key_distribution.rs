use anyhow::Result;
use chrono::{DateTime, Utc};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::crypto::SignedData;
use crate::whitelist::PeerWhitelist;

// Type aliases to reduce complexity
type PendingRequests = Arc<RwLock<HashMap<PeerId, DateTime<Utc>>>>;
type ProcessedMessages = Arc<RwLock<HashMap<String, DateTime<Utc>>>>;

/// Key distribution messages that are exchanged between peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyDistributionMessage {
    /// Request a peer's public key
    KeyRequest {
        requestor: String, // Peer ID of the requestor
        target: String,    // Peer ID whose key is requested
        timestamp: DateTime<Utc>,
    },
    /// Response with a peer's public key
    KeyResponse {
        target: String,      // Peer ID whose key this is
        public_key: Vec<u8>, // Protobuf-encoded public key
        timestamp: DateTime<Utc>,
    },
    /// Announce availability for key exchange
    KeyAnnouncement {
        peer_id: String,
        public_key: Vec<u8>,
        timestamp: DateTime<Utc>,
    },
    /// Request to be added to whitelist with key
    WhitelistRequest {
        peer_id: String,
        public_key: Vec<u8>,
        name: Option<String>,
        timestamp: DateTime<Utc>,
    },
    /// Simple trust recommendation for a peer
    TrustRecommendation {
        recommender: String,  // Peer ID of the recommender
        recommended: String,  // Peer ID being recommended
        name: Option<String>, // Optional name for the recommended peer
        timestamp: DateTime<Utc>,
    },
}

/// Configuration for key distribution behavior
#[derive(Debug, Clone)]
pub struct KeyDistributionConfig {
    /// Whether to automatically share keys with whitelisted peers
    pub auto_share_keys: bool,
    /// Whether to automatically request missing keys
    pub auto_request_keys: bool,
    /// Whether to accept whitelist requests from unknown peers
    pub accept_whitelist_requests: bool,
    /// Maximum age for key distribution messages (in hours)
    pub max_message_age_hours: u64,
}

impl Default for KeyDistributionConfig {
    fn default() -> Self {
        Self {
            auto_share_keys: true,
            auto_request_keys: true,
            accept_whitelist_requests: false, // Conservative default
            max_message_age_hours: 24,
        }
    }
}

/// Manages automated key distribution between peers
pub struct KeyDistributionManager {
    whitelist: Arc<PeerWhitelist>,
    config: KeyDistributionConfig,
    local_keypair: libp2p::identity::Keypair,
    local_peer_id: PeerId,
    /// Track pending key requests to avoid duplicates
    pending_requests: PendingRequests,
    /// Track recently processed messages to avoid replay attacks
    processed_messages: ProcessedMessages,
}

impl KeyDistributionManager {
    pub fn new(
        whitelist: Arc<PeerWhitelist>,
        config: KeyDistributionConfig,
        local_keypair: libp2p::identity::Keypair,
    ) -> Self {
        let local_peer_id = PeerId::from(local_keypair.public());

        Self {
            whitelist,
            config,
            local_keypair,
            local_peer_id,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            processed_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Handle incoming key distribution message
    pub async fn handle_message(
        &self,
        message: SignedData<KeyDistributionMessage>,
        sender_peer_id: PeerId,
    ) -> Result<Option<KeyDistributionMessage>> {
        // Verify message age
        let max_age = chrono::Duration::hours(self.config.max_message_age_hours as i64);
        let message_time = match &message.data {
            KeyDistributionMessage::KeyRequest { timestamp, .. } => *timestamp,
            KeyDistributionMessage::KeyResponse { timestamp, .. } => *timestamp,
            KeyDistributionMessage::KeyAnnouncement { timestamp, .. } => *timestamp,
            KeyDistributionMessage::WhitelistRequest { timestamp, .. } => *timestamp,
            KeyDistributionMessage::TrustRecommendation { timestamp, .. } => *timestamp,
        };

        if Utc::now() - message_time > max_age {
            warn!(
                "Ignoring old key distribution message from {}",
                sender_peer_id
            );
            return Ok(None);
        }

        // Check for replay attacks
        let message_id = format!("{:?}:{:?}", message.data, message.signature);
        {
            let mut processed = self.processed_messages.write().await;
            if processed.contains_key(&message_id) {
                warn!("Ignoring replayed message from {}", sender_peer_id);
                return Ok(None);
            }
            processed.insert(message_id, Utc::now());

            // Clean up old processed messages
            let cutoff = Utc::now() - max_age;
            processed.retain(|_, &mut timestamp| timestamp > cutoff);
        }

        match message.data {
            KeyDistributionMessage::KeyRequest {
                requestor, target, ..
            } => {
                self.handle_key_request(requestor, target, sender_peer_id)
                    .await
            }
            KeyDistributionMessage::KeyResponse {
                target, public_key, ..
            } => {
                self.handle_key_response(target, public_key, sender_peer_id)
                    .await
            }
            KeyDistributionMessage::KeyAnnouncement {
                peer_id,
                public_key,
                ..
            } => {
                self.handle_key_announcement(peer_id, public_key, sender_peer_id)
                    .await
            }
            KeyDistributionMessage::WhitelistRequest {
                peer_id,
                public_key,
                name,
                ..
            } => {
                self.handle_whitelist_request(peer_id, public_key, name, sender_peer_id)
                    .await
            }
            KeyDistributionMessage::TrustRecommendation {
                recommender,
                recommended,
                name,
                ..
            } => {
                self.handle_trust_recommendation(recommender, recommended, name, sender_peer_id)
                    .await
            }
        }
    }

    /// Handle a key request from another peer
    async fn handle_key_request(
        &self,
        requestor: String,
        target: String,
        sender_peer_id: PeerId,
    ) -> Result<Option<KeyDistributionMessage>> {
        let requestor_peer_id = requestor.parse::<PeerId>()?;
        let target_peer_id = target.parse::<PeerId>()?;

        // Only respond if the sender is the requestor and both are whitelisted
        if sender_peer_id != requestor_peer_id {
            warn!(
                "Key request sender mismatch: {} != {}",
                sender_peer_id, requestor_peer_id
            );
            return Ok(None);
        }

        if !self.whitelist.is_whitelisted(&requestor_peer_id).await? {
            warn!(
                "Key request from non-whitelisted peer: {}",
                requestor_peer_id
            );
            return Ok(None);
        }

        // Check if auto key sharing is enabled
        if !self.config.auto_share_keys {
            info!(
                "Auto key sharing is disabled, ignoring key request from: {}",
                requestor_peer_id
            );
            return Ok(None);
        }

        // If they're asking for our key, respond with it
        if target_peer_id == self.local_peer_id {
            let public_key = self.local_keypair.public().encode_protobuf();
            return Ok(Some(KeyDistributionMessage::KeyResponse {
                target: self.local_peer_id.to_string(),
                public_key,
                timestamp: Utc::now(),
            }));
        }

        // If they're asking for another peer's key, check if we have it
        if let Some(public_key) = self.whitelist.get_public_key(&target_peer_id).await? {
            let public_key_bytes = public_key.encode_protobuf();
            return Ok(Some(KeyDistributionMessage::KeyResponse {
                target: target_peer_id.to_string(),
                public_key: public_key_bytes,
                timestamp: Utc::now(),
            }));
        }

        info!(
            "Don't have public key for requested peer: {}",
            target_peer_id
        );
        Ok(None)
    }

    /// Handle a key response from another peer
    async fn handle_key_response(
        &self,
        target: String,
        public_key: Vec<u8>,
        sender_peer_id: PeerId,
    ) -> Result<Option<KeyDistributionMessage>> {
        let target_peer_id = target.parse::<PeerId>()?;

        // Verify the sender is whitelisted
        if !self.whitelist.is_whitelisted(&sender_peer_id).await? {
            warn!("Key response from non-whitelisted peer: {}", sender_peer_id);
            return Ok(None);
        }

        // Decode and verify the public key
        let public_key_obj = libp2p::identity::PublicKey::try_decode_protobuf(&public_key)?;
        let derived_peer_id = PeerId::from(public_key_obj.clone());

        if derived_peer_id != target_peer_id {
            warn!(
                "Public key does not match claimed peer ID: {} != {}",
                derived_peer_id, target_peer_id
            );
            return Ok(None);
        }

        // Check if we had a pending request for this key
        {
            let mut pending = self.pending_requests.write().await;
            if pending.remove(&target_peer_id).is_none() {
                info!("Received unrequested key for peer: {}", target_peer_id);
            }
        }

        // Store the key if the peer is whitelisted
        if self.whitelist.is_whitelisted(&target_peer_id).await? {
            // Get existing entry details to preserve name and expiration
            let entries = self.whitelist.list_peers().await?;
            let entry = entries
                .iter()
                .find(|e| e.peer_id == target_peer_id.to_string());

            if let Some(entry) = entry {
                self.whitelist
                    .add_peer(
                        &target_peer_id,
                        entry.name.clone(),
                        Some(&public_key_obj),
                        entry.expires_at,
                    )
                    .await?;
                info!(
                    "Updated public key for whitelisted peer: {}",
                    target_peer_id
                );
            }
        }

        Ok(None)
    }

    /// Handle a key announcement from another peer
    async fn handle_key_announcement(
        &self,
        peer_id: String,
        public_key: Vec<u8>,
        sender_peer_id: PeerId,
    ) -> Result<Option<KeyDistributionMessage>> {
        let announced_peer_id = peer_id.parse::<PeerId>()?;

        // Verify the sender is announcing their own key
        if sender_peer_id != announced_peer_id {
            warn!(
                "Key announcement peer ID mismatch: {} != {}",
                sender_peer_id, announced_peer_id
            );
            return Ok(None);
        }

        // Verify the sender is whitelisted
        if !self.whitelist.is_whitelisted(&sender_peer_id).await? {
            warn!(
                "Key announcement from non-whitelisted peer: {}",
                sender_peer_id
            );
            return Ok(None);
        }

        // Decode and verify the public key
        let public_key_obj = libp2p::identity::PublicKey::try_decode_protobuf(&public_key)?;
        let derived_peer_id = PeerId::from(public_key_obj.clone());

        if derived_peer_id != announced_peer_id {
            warn!(
                "Announced public key does not match peer ID: {} != {}",
                derived_peer_id, announced_peer_id
            );
            return Ok(None);
        }

        // Update the peer's public key
        let entries = self.whitelist.list_peers().await?;
        let entry = entries
            .iter()
            .find(|e| e.peer_id == announced_peer_id.to_string());

        if let Some(entry) = entry {
            self.whitelist
                .add_peer(
                    &announced_peer_id,
                    entry.name.clone(),
                    Some(&public_key_obj),
                    entry.expires_at,
                )
                .await?;
            info!(
                "Updated public key from announcement: {}",
                announced_peer_id
            );
        }

        Ok(None)
    }

    /// Handle a whitelist request from an unknown peer
    async fn handle_whitelist_request(
        &self,
        peer_id: String,
        public_key: Vec<u8>,
        name: Option<String>,
        sender_peer_id: PeerId,
    ) -> Result<Option<KeyDistributionMessage>> {
        if !self.config.accept_whitelist_requests {
            info!(
                "Whitelist requests are disabled, ignoring request from: {}",
                sender_peer_id
            );
            return Ok(None);
        }

        let requested_peer_id = peer_id.parse::<PeerId>()?;

        // Verify the sender is requesting for themselves
        if sender_peer_id != requested_peer_id {
            warn!(
                "Whitelist request peer ID mismatch: {} != {}",
                sender_peer_id, requested_peer_id
            );
            return Ok(None);
        }

        // Decode and verify the public key
        let public_key_obj = libp2p::identity::PublicKey::try_decode_protobuf(&public_key)?;
        let derived_peer_id = PeerId::from(public_key_obj.clone());

        if derived_peer_id != requested_peer_id {
            warn!(
                "Whitelist request public key does not match peer ID: {} != {}",
                derived_peer_id, requested_peer_id
            );
            return Ok(None);
        }

        info!(
            "Received whitelist request from: {} (name: {:?})",
            sender_peer_id, name
        );

        // Note: This is a security-sensitive operation that might require manual approval
        // For now, we just log it. In a production system, this might trigger notifications
        // or require administrator approval.

        Ok(None)
    }

    /// Handle a trust recommendation from another peer
    async fn handle_trust_recommendation(
        &self,
        recommender: String,
        recommended: String,
        name: Option<String>,
        sender_peer_id: PeerId,
    ) -> Result<Option<KeyDistributionMessage>> {
        let recommender_peer_id = recommender.parse::<PeerId>()?;
        let recommended_peer_id = recommended.parse::<PeerId>()?;

        // Verify the sender is the recommender
        if sender_peer_id != recommender_peer_id {
            warn!(
                "Trust recommendation sender mismatch: {} != {}",
                sender_peer_id, recommender_peer_id
            );
            return Ok(None);
        }

        // Verify the recommender is whitelisted
        if !self.whitelist.is_whitelisted(&recommender_peer_id).await? {
            warn!(
                "Trust recommendation from non-whitelisted peer: {}",
                recommender_peer_id
            );
            return Ok(None);
        }

        // Don't allow self-recommendation
        if recommender_peer_id == recommended_peer_id {
            warn!(
                "Peer {} attempted to recommend themselves",
                recommender_peer_id
            );
            return Ok(None);
        }

        // Add the recommendation
        match self
            .whitelist
            .add_recommendation(&recommended_peer_id, &recommender_peer_id, name.clone())
            .await
        {
            Ok(()) => {
                info!(
                    "Added trust recommendation: {} recommended by {}",
                    recommended_peer_id, recommender_peer_id
                );
            }
            Err(e) => {
                warn!("Failed to add trust recommendation: {}", e);
            }
        }

        Ok(None)
    }

    /// Request missing public keys for whitelisted peers
    pub async fn request_missing_keys(&self) -> Result<Vec<KeyDistributionMessage>> {
        if !self.config.auto_request_keys {
            return Ok(Vec::new());
        }

        let mut requests = Vec::new();
        let entries = self.whitelist.list_peers().await?;

        for entry in entries {
            if entry.public_key.is_none() {
                let peer_id = entry.peer_id.parse::<PeerId>()?;

                // Check if we already have a pending request
                {
                    let pending = self.pending_requests.read().await;
                    if let Some(&request_time) = pending.get(&peer_id) {
                        if Utc::now() - request_time < chrono::Duration::minutes(5) {
                            continue; // Skip recent requests
                        }
                    }
                }

                // Add to pending requests
                {
                    let mut pending = self.pending_requests.write().await;
                    pending.insert(peer_id, Utc::now());
                }

                requests.push(KeyDistributionMessage::KeyRequest {
                    requestor: self.local_peer_id.to_string(),
                    target: peer_id.to_string(),
                    timestamp: Utc::now(),
                });

                info!("Requesting public key for peer: {}", peer_id);
            }
        }

        Ok(requests)
    }

    /// Announce our public key to whitelisted peers
    pub fn create_key_announcement(&self) -> KeyDistributionMessage {
        let public_key = self.local_keypair.public().encode_protobuf();

        KeyDistributionMessage::KeyAnnouncement {
            peer_id: self.local_peer_id.to_string(),
            public_key,
            timestamp: Utc::now(),
        }
    }

    /// Create a whitelist request message
    pub fn create_whitelist_request(&self, name: Option<String>) -> KeyDistributionMessage {
        let public_key = self.local_keypair.public().encode_protobuf();

        KeyDistributionMessage::WhitelistRequest {
            peer_id: self.local_peer_id.to_string(),
            public_key,
            name,
            timestamp: Utc::now(),
        }
    }

    /// Get the local keypair for signing messages
    pub fn local_keypair(&self) -> &libp2p::identity::Keypair {
        &self.local_keypair
    }

    /// Clean up old pending requests and processed messages
    pub async fn cleanup(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::hours(1);

        {
            let mut pending = self.pending_requests.write().await;
            pending.retain(|_, &mut timestamp| timestamp > cutoff);
        }

        {
            let mut processed = self.processed_messages.write().await;
            processed.retain(|_, &mut timestamp| timestamp > cutoff);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_key_distribution_manager_creation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("whitelist.db");
        #[allow(clippy::arc_with_non_send_sync)]
        let whitelist = Arc::new(PeerWhitelist::new(&db_path).unwrap());

        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let config = KeyDistributionConfig::default();

        let _manager = KeyDistributionManager::new(whitelist, config, keypair);
    }

    #[tokio::test]
    async fn test_key_announcement_creation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("whitelist.db");
        #[allow(clippy::arc_with_non_send_sync)]
        let whitelist = Arc::new(PeerWhitelist::new(&db_path).unwrap());

        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let config = KeyDistributionConfig::default();

        let manager = KeyDistributionManager::new(whitelist, config, keypair);
        let announcement = manager.create_key_announcement();

        match announcement {
            KeyDistributionMessage::KeyAnnouncement {
                peer_id,
                public_key,
                ..
            } => {
                assert_eq!(peer_id, manager.local_peer_id.to_string());
                assert!(!public_key.is_empty());
            }
            _ => panic!("Expected KeyAnnouncement"),
        }
    }

    #[tokio::test]
    async fn test_missing_keys_request() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("whitelist.db");
        #[allow(clippy::arc_with_non_send_sync)]
        let whitelist = Arc::new(PeerWhitelist::new(&db_path).unwrap());

        // Add a peer without public key
        let peer_id = PeerId::random();
        whitelist
            .add_peer(&peer_id, Some("Test Peer".to_string()), None, None)
            .await
            .unwrap();

        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let config = KeyDistributionConfig::default();

        let manager = KeyDistributionManager::new(whitelist, config, keypair);
        let requests = manager.request_missing_keys().await.unwrap();

        assert_eq!(requests.len(), 1);
        match &requests[0] {
            KeyDistributionMessage::KeyRequest { target, .. } => {
                assert_eq!(target, &peer_id.to_string());
            }
            _ => panic!("Expected KeyRequest"),
        }
    }
}

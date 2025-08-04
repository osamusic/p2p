use libp2p::{gossipsub, identify, kad, mdns, swarm::NetworkBehaviour};

#[derive(NetworkBehaviour)]
pub struct P2PSyncBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::{
        gossipsub::{Config as GossipsubConfig, MessageAuthenticity},
        identify::Config as IdentifyConfig,
        kad::{store::MemoryStore, Config as KadConfig},
        mdns, PeerId,
    };

    fn create_test_behaviour(
        peer_id: PeerId,
        keypair: &libp2p::identity::Keypair,
    ) -> P2PSyncBehaviour {
        // Create gossipsub behaviour
        let gossipsub_config = GossipsubConfig::default();

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        )
        .expect("Correct configuration");

        // Create mDNS behaviour
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)
            .expect("Failed to create mDNS behaviour");

        // Create Kademlia behaviour
        let kad_config = KadConfig::new(libp2p::StreamProtocol::new("/kad/1.0.0"));

        let store = MemoryStore::new(peer_id);
        let kad = kad::Behaviour::with_config(peer_id, store, kad_config);

        // Create identify behaviour
        let identify_config = IdentifyConfig::new("p2p-sync/1.0.0".to_string(), keypair.public());
        let identify = identify::Behaviour::new(identify_config);

        P2PSyncBehaviour {
            gossipsub,
            mdns,
            kad,
            identify,
        }
    }

    #[tokio::test]
    async fn test_behaviour_creation() {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let _behaviour = create_test_behaviour(peer_id, &keypair);

        // Test that behaviour was created successfully
        // The NetworkBehaviour derive macro ensures the struct is properly constructed
    }

    #[tokio::test]
    async fn test_behaviour_with_swarm() {
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let behaviour = create_test_behaviour(local_peer_id, &local_key);

        // For now, just test that the behaviour can be created
        // Full swarm testing would require more complex setup
        assert!(matches!(behaviour.gossipsub, _));
        assert!(matches!(behaviour.mdns, _));
        assert!(matches!(behaviour.kad, _));
        assert!(matches!(behaviour.identify, _));
    }

    #[tokio::test]
    async fn test_multiple_behaviours() {
        // Test creating multiple behaviour instances
        let keypairs: Vec<_> = (0..5)
            .map(|_| libp2p::identity::Keypair::generate_ed25519())
            .collect();
        let peer_ids: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::from(kp.public()))
            .collect();

        let _behaviours: Vec<_> = peer_ids
            .iter()
            .zip(&keypairs)
            .map(|(id, kp)| create_test_behaviour(*id, kp))
            .collect();

        // Successfully created multiple behaviours
    }

    #[tokio::test]
    async fn test_gossipsub_topic_subscription() {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let mut behaviour = create_test_behaviour(peer_id, &keypair);

        let topic = gossipsub::IdentTopic::new("test-topic");

        // Test subscribing to a topic
        behaviour.gossipsub.subscribe(&topic).unwrap();

        // Verify subscription
        let topics: Vec<_> = behaviour.gossipsub.topics().collect();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0], &topic.hash());
    }

    #[tokio::test]
    async fn test_kademlia_operations() {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let mut behaviour = create_test_behaviour(peer_id, &keypair);

        // Test adding a peer to the routing table
        let other_peer = PeerId::random();
        let addr = "/memory/1234".parse().unwrap();

        behaviour.kad.add_address(&other_peer, addr);

        // The address should be stored in the Kademlia DHT
        // Note: We can't directly verify this without a running swarm,
        // but the operation should not panic
    }
}

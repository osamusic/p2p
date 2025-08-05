use libp2p::PeerId;
use p2p_sync::whitelist::PeerWhitelist;
use tempfile::tempdir;

#[tokio::test]
async fn test_whitelist_integration() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("whitelist.db");
    let whitelist = PeerWhitelist::new(&db_path).unwrap();

    // Generate test peer IDs
    let peer1 = PeerId::random();
    let peer2 = PeerId::random();
    let peer3 = PeerId::random();

    // Test adding peers to whitelist
    whitelist
        .add_peer(&peer1, Some("Node 1".to_string()), None, None)
        .await
        .unwrap();
    whitelist
        .add_peer(
            &peer2,
            Some("Node 2".to_string()),
            None,
            Some(chrono::Utc::now() + chrono::Duration::hours(24)),
        )
        .await
        .unwrap();

    // Test checking whitelist
    assert!(whitelist.is_whitelisted(&peer1).await.unwrap());
    assert!(whitelist.is_whitelisted(&peer2).await.unwrap());
    assert!(!whitelist.is_whitelisted(&peer3).await.unwrap());

    // Test listing peers
    let peers = whitelist.list_peers().await.unwrap();
    assert_eq!(peers.len(), 2);

    // Test removing peer
    whitelist.remove_peer(&peer1).await.unwrap();
    assert!(!whitelist.is_whitelisted(&peer1).await.unwrap());

    // Test expired peer
    let expired_peer = PeerId::random();
    whitelist
        .add_peer(
            &expired_peer,
            None,
            None,
            Some(chrono::Utc::now() - chrono::Duration::hours(1)),
        )
        .await
        .unwrap();
    assert!(!whitelist.is_whitelisted(&expired_peer).await.unwrap());
}

#[tokio::test]
async fn test_whitelist_persistence() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("whitelist.db");

    let peer_id = PeerId::random();

    // Create whitelist and add peer
    {
        let whitelist = PeerWhitelist::new(&db_path).unwrap();
        whitelist
            .add_peer(&peer_id, Some("Persistent Peer".to_string()), None, None)
            .await
            .unwrap();
    }

    // Create new whitelist instance and check peer is still there
    {
        let whitelist = PeerWhitelist::new(&db_path).unwrap();
        assert!(whitelist.is_whitelisted(&peer_id).await.unwrap());

        let peers = whitelist.list_peers().await.unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].name, Some("Persistent Peer".to_string()));
    }
}

#[tokio::test]
async fn test_whitelist_with_public_key() {
    use libp2p::identity;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("whitelist.db");
    let whitelist = PeerWhitelist::new(&db_path).unwrap();

    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    let public_key = keypair.public();

    // Add peer with public key
    whitelist
        .add_peer(
            &peer_id,
            Some("Node with Key".to_string()),
            Some(&public_key),
            None,
        )
        .await
        .unwrap();

    // Check peer is whitelisted
    assert!(whitelist.is_whitelisted(&peer_id).await.unwrap());

    // Check public key can be retrieved
    let stored_key = whitelist.get_public_key(&peer_id).await.unwrap();
    assert!(stored_key.is_some());

    let stored_key = stored_key.unwrap();
    assert_eq!(stored_key.to_peer_id(), peer_id);

    // Test listing shows public key info
    let entries = whitelist.list_peers().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].public_key.is_some());
}

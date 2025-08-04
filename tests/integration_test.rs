use p2p_sync::{security::SecurityConfig, storage::Storage};
use tempfile::tempdir;

#[tokio::test]
async fn test_storage_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let storage = Storage::new(&db_path).expect("Failed to create storage");

    // テスト: put and get
    storage.put("key1", "value1").expect("Failed to put");
    let value = storage.get("key1").expect("Failed to get");
    assert_eq!(value, Some("value1".to_string()));

    // テスト: non-existent key
    let value = storage.get("non_existent").expect("Failed to get");
    assert_eq!(value, None);

    // テスト: list all items
    storage.put("key2", "value2").expect("Failed to put");
    let items = storage.list().expect("Failed to list");
    assert_eq!(items.len(), 2);
    assert!(items.contains(&("key1".to_string(), "value1".to_string())));
    assert!(items.contains(&("key2".to_string(), "value2".to_string())));
}

#[test]
fn test_security_config_default() {
    let config = SecurityConfig::default();

    assert_eq!(config.rate_limit_per_minute, 60);
    assert_eq!(config.rate_limit_burst, 10);
    assert_eq!(config.max_message_size, 1024 * 1024);
    assert_eq!(config.max_key_length, 256);
    assert_eq!(config.max_value_length, 1024 * 64);
    assert_eq!(config.max_connections_per_ip, 10);
    assert!(config.blocked_peers.is_empty());
    assert!(config.allowed_peers.is_none());
}

#[test]
fn test_config_loading() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // テスト: default config loading when file doesn't exist
    let config = p2p_sync::config::load_config(&config_path).expect("Failed to load config");
    assert_eq!(config.port, 4001);
    assert!(config.bootstrap_peers.is_empty());
}

#[test]
fn test_input_validation() {
    use p2p_sync::security::{validate_key, validate_value};

    // Valid inputs
    assert!(validate_key("valid_key", 256).is_ok());
    assert!(validate_value("valid_value", 1024).is_ok());

    // Invalid inputs
    assert!(validate_key("", 256).is_err()); // empty key
    assert!(validate_key(&"x".repeat(300), 256).is_err()); // too long
    assert!(validate_key("../path", 256).is_err()); // path traversal
    assert!(validate_key("key\x00", 256).is_err()); // control character

    assert!(validate_value(&"x".repeat(2000), 1024).is_err()); // too long
}

#[tokio::test]
async fn test_rate_limiter() {
    use libp2p::PeerId;
    use p2p_sync::security::{RateLimiter, SecurityConfig};

    let mut config = SecurityConfig::default();
    config.rate_limit_per_minute = 2;
    config.rate_limit_burst = 1;

    let rate_limiter = RateLimiter::new(config);
    let peer_id = PeerId::random();

    // First request should be allowed
    assert!(rate_limiter.check_rate_limit(&peer_id).await.is_ok());

    // Second request should hit burst limit
    assert!(rate_limiter.check_rate_limit(&peer_id).await.is_err());
}

#[tokio::test]
async fn test_access_control() {
    use libp2p::PeerId;
    use p2p_sync::security::{AccessControl, SecurityConfig};
    use std::collections::HashSet;

    let peer_id = PeerId::random();
    let peer_str = peer_id.to_string();

    // Test blocked peer
    let mut config = SecurityConfig::default();
    config.blocked_peers.insert(peer_str.clone());

    let access_control = AccessControl::new(config);
    assert!(access_control.check_peer_allowed(&peer_id).await.is_err());

    // Test allowed peer with whitelist
    let mut config = SecurityConfig::default();
    let mut allowed_peers = HashSet::new();
    allowed_peers.insert(peer_str);
    config.allowed_peers = Some(allowed_peers);

    let access_control = AccessControl::new(config);
    assert!(access_control.check_peer_allowed(&peer_id).await.is_ok());
}

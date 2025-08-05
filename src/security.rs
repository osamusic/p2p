use anyhow::{bail, Result};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::whitelist::PeerWhitelist;

type RequestMap = Arc<RwLock<HashMap<PeerId, Vec<Instant>>>>;
type ConnectionMap = Arc<RwLock<HashMap<IpAddr, usize>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    // レート制限設定
    pub rate_limit_per_minute: u32,
    pub rate_limit_burst: u32,

    // メッセージサイズ制限
    pub max_message_size: usize,
    pub max_key_length: usize,
    pub max_value_length: usize,

    // 接続制限
    pub max_connections_per_ip: usize,
    pub connection_timeout: Duration,

    // ブロックリスト
    pub blocked_peers: HashSet<String>,
    pub allowed_peers: Option<HashSet<String>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            rate_limit_per_minute: 60,
            rate_limit_burst: 10,
            max_message_size: 1024 * 1024, // 1MB
            max_key_length: 256,
            max_value_length: 1024 * 64, // 64KB
            max_connections_per_ip: 10,
            connection_timeout: Duration::from_secs(30),
            blocked_peers: HashSet::new(),
            allowed_peers: None,
        }
    }
}

pub struct RateLimiter {
    requests: RequestMap,
    config: SecurityConfig,
}

impl RateLimiter {
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn check_rate_limit(&self, peer_id: &PeerId) -> Result<()> {
        let now = Instant::now();
        let minute_ago = now - Duration::from_secs(60);

        let mut requests = self.requests.write().await;
        let peer_requests = requests.entry(*peer_id).or_insert_with(Vec::new);

        // 1分以上前のリクエストを削除
        peer_requests.retain(|&instant| instant > minute_ago);

        // レート制限チェック
        if peer_requests.len() >= self.config.rate_limit_per_minute as usize {
            bail!("Rate limit exceeded for peer: {}", peer_id);
        }

        // バーストチェック
        let recent_requests = peer_requests
            .iter()
            .filter(|&&instant| instant > now - Duration::from_secs(1))
            .count();

        if recent_requests >= self.config.rate_limit_burst as usize {
            bail!("Burst limit exceeded for peer: {}", peer_id);
        }

        peer_requests.push(now);
        Ok(())
    }
}

pub struct AccessControl {
    config: SecurityConfig,
    connections_per_ip: ConnectionMap,
    whitelist: Option<Arc<PeerWhitelist>>,
}

impl AccessControl {
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            connections_per_ip: Arc::new(RwLock::new(HashMap::new())),
            whitelist: None,
        }
    }

    pub fn with_whitelist(config: SecurityConfig, whitelist: Arc<PeerWhitelist>) -> Self {
        Self {
            config,
            connections_per_ip: Arc::new(RwLock::new(HashMap::new())),
            whitelist: Some(whitelist),
        }
    }

    pub async fn check_peer_allowed(&self, peer_id: &PeerId) -> Result<()> {
        let peer_str = peer_id.to_string();

        // ブロックリストチェック
        if self.config.blocked_peers.contains(&peer_str) {
            bail!("Peer is blocked: {}", peer_id);
        }

        // データベースベースのホワイトリストチェック（設定されている場合）
        if let Some(whitelist) = &self.whitelist {
            if !whitelist.is_whitelisted(peer_id).await? {
                bail!("Peer not in whitelist: {}", peer_id);
            }
        }
        // 設定ベースのホワイトリストチェック（後方互換性のため）
        else if let Some(allowed) = &self.config.allowed_peers {
            if !allowed.contains(&peer_str) {
                bail!("Peer not in allowed list: {}", peer_id);
            }
        }

        Ok(())
    }

    pub async fn check_connection_limit(&self, ip: &IpAddr) -> Result<()> {
        let mut connections = self.connections_per_ip.write().await;
        let count = connections.entry(*ip).or_insert(0);

        if *count >= self.config.max_connections_per_ip {
            bail!("Connection limit exceeded for IP: {}", ip);
        }

        *count += 1;
        Ok(())
    }

    pub async fn release_connection(&self, ip: &IpAddr) {
        let mut connections = self.connections_per_ip.write().await;
        if let Some(count) = connections.get_mut(ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(ip);
            }
        }
    }
}

pub fn validate_key(key: &str, max_length: usize) -> Result<()> {
    if key.is_empty() {
        bail!("Key cannot be empty");
    }

    if key.len() > max_length {
        bail!("Key too long: {} > {}", key.len(), max_length);
    }

    // 制御文字のチェック
    if key
        .chars()
        .any(|c| c.is_control() && c != '\t' && c != '\n')
    {
        bail!("Key contains invalid control characters");
    }

    // パストラバーサル攻撃の防止
    if key.contains("..") || key.contains("//") || key.starts_with('/') {
        bail!("Key contains potentially unsafe path characters");
    }

    Ok(())
}

pub fn validate_value(value: &str, max_length: usize) -> Result<()> {
    if value.len() > max_length {
        bail!("Value too long: {} > {}", value.len(), max_length);
    }

    Ok(())
}

pub fn sanitize_input(input: &str) -> String {
    input
        .chars()
        .filter(|&c| !c.is_control() || c == '\t' || c == '\n')
        .take(1024) // 最大1024文字
        .collect()
}

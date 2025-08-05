use anyhow::Result;
use clap::{Parser, Subcommand};
use futures::StreamExt;
use libp2p::{gossipsub, identify, kad, mdns, noise, tcp, yamux, Multiaddr, SwarmBuilder};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tracing::info;

mod autostart;
mod config;
mod connection_manager;
mod crypto;
mod key_distribution;
mod network;
mod security;
mod storage;
mod sync;
mod whitelist;

use connection_manager::ConnectionManager;
use crypto::SignedData;
use key_distribution::{KeyDistributionConfig, KeyDistributionManager, KeyDistributionMessage};
use network::P2PSyncBehaviour;
use security::{
    sanitize_input, validate_key, validate_value, AccessControl, RateLimiter, SecurityConfig,
};
use storage::Storage;
use sync::{P2PMessage, SyncMessage};
use whitelist::PeerWhitelist;

#[derive(Parser)]
#[command(name = "p2p-sync")]
#[command(about = "P2P synchronization system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start {
        #[arg(short, long, default_value_t = 0)]
        port: u16,

        #[arg(short, long)]
        dial: Option<Multiaddr>,

        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },

    Install,

    #[command(subcommand)]
    Whitelist(WhitelistCommands),
}

#[derive(Subcommand)]
enum WhitelistCommands {
    Add {
        peer_id: String,
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        expires_in_hours: Option<u64>,
        #[arg(short = 'k', long)]
        public_key_file: Option<String>,
    },

    Remove {
        peer_id: String,
    },

    List,

    Check {
        peer_id: String,
    },

    AddKey {
        peer_id: String,
        public_key_file: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            port,
            dial,
            data_dir,
        } => {
            start_node(port, dial, data_dir).await?;
        }
        Commands::Install => {
            install_service()?;
        }
        Commands::Whitelist(cmd) => {
            handle_whitelist_command(cmd).await?;
        }
    }

    Ok(())
}

async fn start_node(
    port: u16,
    dial_addr: Option<Multiaddr>,
    data_dir: Option<PathBuf>,
) -> Result<()> {
    let data_dir = data_dir.unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("p2p-sync")
    });

    std::fs::create_dir_all(&data_dir)?;
    let storage = Storage::new(data_dir.join("sync.db"))?;

    // 設定の読み込み
    let config_path = data_dir.join("config.toml");
    let config = config::load_config(&config_path)?;

    // デフォルト設定を保存
    if !config_path.exists() {
        config::save_config(&config_path, &config)?;
        info!("Created default config at: {}", config_path.display());
    }

    let rate_limiter = RateLimiter::new(config.security.clone());

    // ホワイトリストの初期化
    let whitelist_path = data_dir.join("whitelist.db");
    #[allow(clippy::arc_with_non_send_sync)]
    let whitelist = Arc::new(PeerWhitelist::new(&whitelist_path)?);

    // ホワイトリストを含むアクセス制御の初期化
    let access_control = AccessControl::with_whitelist(config.security.clone(), whitelist.clone());
    let connection_manager = ConnectionManager::new(access_control);

    // Generate keypair for this node
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = libp2p::PeerId::from(local_key.public());

    // Initialize key distribution manager
    let key_dist_config = KeyDistributionConfig::default();
    #[allow(clippy::arc_with_non_send_sync)]
    let key_dist_manager = Arc::new(KeyDistributionManager::new(
        whitelist.clone(),
        key_dist_config,
        local_key.clone(),
    ));

    let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .expect("Valid config");

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .expect("Correct configuration");

            let mdns =
                mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
            let kad = kad::Behaviour::new(
                key.public().to_peer_id(),
                kad::store::MemoryStore::new(key.public().to_peer_id()),
            );
            let identify = identify::Behaviour::new(identify::Config::new(
                "/p2p-sync/0.1.0".to_string(),
                key.public(),
            ));

            Ok(P2PSyncBehaviour {
                gossipsub,
                mdns,
                kad,
                identify,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let topic = gossipsub::IdentTopic::new("p2p-sync");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{port}").parse()?)?;
    swarm.listen_on(format!("/ip4/0.0.0.0/udp/{port}/quic-v1").parse()?)?;

    if let Some(addr) = dial_addr {
        swarm.dial(addr)?;
    }

    info!("Local peer id: {:?}", swarm.local_peer_id());

    // 初期プロンプトを表示
    println!("\n=== P2P Sync System Started ===");
    println!("Local Peer ID: {local_peer_id}");
    println!("Commands: add <key> <value>, get <key>, delete <key>, list, status");
    println!("Press Ctrl+C to exit\n");
    print!("> ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Ok(Some(line)) = line {
                    handle_input(&mut swarm, &storage, &topic, line, &config.security, &connection_manager, &local_key, &key_dist_manager, &whitelist).await?;
                    // 次のプロンプトを表示
                    print!("> ");
                    std::io::stdout().flush()?;
                }
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, &storage, &topic, event, &rate_limiter, &connection_manager, &whitelist, &key_dist_manager).await?;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_input(
    swarm: &mut libp2p::Swarm<P2PSyncBehaviour>,
    storage: &Storage,
    topic: &gossipsub::IdentTopic,
    input: String,
    security_config: &SecurityConfig,
    connection_manager: &ConnectionManager,
    local_key: &libp2p::identity::Keypair,
    key_dist_manager: &Arc<KeyDistributionManager>,
    whitelist: &Arc<PeerWhitelist>,
) -> Result<()> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    match parts.as_slice() {
        ["add", key, value] => {
            // 入力のサニタイズ
            let sanitized_key = sanitize_input(key);
            let sanitized_value = sanitize_input(value);

            // 入力検証
            validate_key(&sanitized_key, security_config.max_key_length)?;
            validate_value(&sanitized_value, security_config.max_value_length)?;

            let timestamp = chrono::Utc::now();
            let msg = SyncMessage::Put {
                key: sanitized_key.clone(),
                value: sanitized_value.clone(),
                timestamp,
            };

            storage.put(&sanitized_key, &sanitized_value)?;

            // Convert to P2P message and sign
            let p2p_msg = P2PMessage::Sync(msg);
            let signed_data = SignedData::new(p2p_msg, local_key)?;

            let json = serde_json::to_vec(&signed_data)?;

            // メッセージサイズチェック
            if json.len() > security_config.max_message_size {
                anyhow::bail!("Message too large: {} bytes", json.len());
            }

            swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), json)?;

            println!("✓ Added: {sanitized_key} = {sanitized_value}");
            info!("Published: {} = {}", sanitized_key, sanitized_value);
        }
        ["get", key] => match storage.get(key)? {
            Some(value) => {
                println!("✓ {key} = {value}");
                info!("{} = {}", key, value);
            }
            None => {
                println!("✗ {key} not found");
                info!("{} not found", key);
            }
        },
        ["list"] => {
            let items = storage.list()?;
            if items.is_empty() {
                println!("No items stored");
            } else {
                println!("Stored items ({}):", items.len());
                for (key, value) in items {
                    println!("  {key} = {value}");
                }
            }
        }
        ["status"] => {
            let connection_count = connection_manager.get_connection_count().await;
            let active_connections = connection_manager.get_active_connections().await;
            println!("=== P2P Status ===");
            println!("Active connections: {connection_count}");
            if connection_count > 0 {
                println!("Connected peers:");
                for (peer_id, ip) in active_connections {
                    println!("  {peer_id} <- {ip}");
                }
            } else {
                println!("No active connections - waiting for peers...");
            }
            info!("Status checked - {} active connections", connection_count);
        }
        ["delete", key] => {
            let timestamp = chrono::Utc::now();
            let msg = SyncMessage::Delete {
                key: key.to_string(),
                timestamp,
            };

            storage.delete_with_timestamp(key, timestamp)?;

            // Convert to P2P message and sign
            let p2p_msg = P2PMessage::Sync(msg);
            let signed_data = SignedData::new(p2p_msg, local_key)?;

            let json = serde_json::to_vec(&signed_data)?;

            // メッセージサイズチェック
            if json.len() > security_config.max_message_size {
                anyhow::bail!("Message too large: {} bytes", json.len());
            }

            swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), json)?;

            println!("✓ Deleted: {key}");
            info!("Deleted: {}", key);
        }
        ["help"] | ["h"] => {
            println!("Available commands:");
            println!("  add <key> <value>  - Add or update a key-value pair");
            println!("  get <key>          - Retrieve value for a key");
            println!("  delete <key>       - Delete a key-value pair");
            println!("  list               - List all stored items");
            println!("  status             - Show connection status");
            println!("  peers              - Show connected peers");
            println!("  info               - Show node information");
            println!("  help               - Show this help message");
            println!();
            println!("Whitelist Management (run separately):");
            println!("  p2p-sync whitelist add <peer_id> [-n name] [-e hours] [-k key_file]");
            println!("  p2p-sync whitelist remove <peer_id>");
            println!("  p2p-sync whitelist list");
            println!("  p2p-sync whitelist check <peer_id>");
            println!("  p2p-sync whitelist add-key <peer_id> <public_key_file>");
            println!();
            println!("Key Distribution (interactive commands):");
            println!("  announce-key       - Announce your public key to all peers");
            println!("  request-keys       - Request missing public keys");
            println!("  request-whitelist  - Request to be added to peer whitelists");
            println!();
            println!("Trust Management:");
            println!("  recommend-peer <peer_id> - Recommend a peer to the network");
            println!();
            println!("Maintenance:");
            println!("  cleanup - Clean up old key distribution data");
            println!("  reload-cache - Reload whitelist cache from database");
        }
        ["peers"] => {
            let active_connections = connection_manager.get_active_connections().await;
            if active_connections.is_empty() {
                println!("No connected peers");
            } else {
                println!("Connected peers ({}):", active_connections.len());
                for (peer_id, ip) in active_connections {
                    println!("  {peer_id} from {ip}");
                }
            }
        }
        ["info"] => {
            let local_peer_id = swarm.local_peer_id();
            let listeners: Vec<_> = swarm.listeners().collect();
            println!("=== Node Information ===");
            println!("Local Peer ID: {local_peer_id}");
            println!("Listening on:");
            for addr in listeners {
                println!("  {addr}");
            }
            let connection_count = connection_manager.get_connection_count().await;
            println!("Active connections: {connection_count}");
        }
        ["announce-key"] => {
            let announcement = key_dist_manager.create_key_announcement();
            let p2p_msg = P2PMessage::KeyDistribution(announcement);
            let signed_data = SignedData::new(p2p_msg, local_key)?;

            let json = serde_json::to_vec(&signed_data)?;
            if json.len() > security_config.max_message_size {
                anyhow::bail!("Message too large: {} bytes", json.len());
            }

            swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), json)?;
            println!("✓ Announced public key to all peers");
            info!("Published key announcement");
        }
        ["request-keys"] => {
            let requests = key_dist_manager.request_missing_keys().await?;

            if requests.is_empty() {
                println!("No missing keys to request");
            } else {
                let num_requests = requests.len();
                for request in requests {
                    let p2p_msg = P2PMessage::KeyDistribution(request);
                    let signed_data = SignedData::new(p2p_msg, local_key)?;

                    let json = serde_json::to_vec(&signed_data)?;
                    if json.len() <= security_config.max_message_size {
                        swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topic.clone(), json)?;
                    }
                }
                println!("✓ Requested {num_requests} missing public key(s)");
                info!("Published {} key requests", num_requests);
            }
        }
        ["request-whitelist"] => {
            print!("Enter your name (optional): ");
            std::io::stdout().flush()?;

            let mut name_input = String::new();
            if std::io::stdin().read_line(&mut name_input).is_ok() {
                let name = name_input.trim();
                let name = if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                };

                let request = key_dist_manager.create_whitelist_request(name);
                let p2p_msg = P2PMessage::KeyDistribution(request);
                let signed_data = SignedData::new(p2p_msg, local_key)?;

                let json = serde_json::to_vec(&signed_data)?;
                if json.len() > security_config.max_message_size {
                    anyhow::bail!("Message too large: {} bytes", json.len());
                }

                swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic.clone(), json)?;
                println!("✓ Sent whitelist request to all peers");
                info!("Published whitelist request");
            }
        }
        ["recommend-peer", peer_id] => {
            // Parse peer ID
            let peer_id = match peer_id.parse::<libp2p::PeerId>() {
                Ok(id) => id,
                Err(_) => {
                    println!("✗ Invalid peer ID format");
                    return Ok(());
                }
            };

            print!("Enter optional name for this peer: ");
            std::io::stdout().flush()?;

            let mut name_input = String::new();
            if std::io::stdin().read_line(&mut name_input).is_ok() {
                let name = name_input.trim();
                let name = if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                };

                let recommendation = KeyDistributionMessage::TrustRecommendation {
                    recommender: swarm.local_peer_id().to_string(),
                    recommended: peer_id.to_string(),
                    name: name.clone(),
                    timestamp: chrono::Utc::now(),
                };

                let p2p_msg = P2PMessage::KeyDistribution(recommendation);
                let signed_data = SignedData::new(p2p_msg, local_key)?;

                let json = serde_json::to_vec(&signed_data)?;
                if json.len() > security_config.max_message_size {
                    anyhow::bail!("Message too large: {} bytes", json.len());
                }

                swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic.clone(), json)?;
                println!("✓ Recommended peer {peer_id} to the network");
                info!("Published trust recommendation for {}", peer_id);
            }
        }
        ["cleanup"] => {
            key_dist_manager.cleanup().await?;
            println!("✓ Cleaned up old key distribution data");
            info!("Performed key distribution cleanup");
        }
        ["reload-cache"] => {
            whitelist.reload_cache().await?;
            println!("✓ Reloaded whitelist cache");
            info!("Reloaded whitelist cache");
        }
        ["verify-signature"] => {
            // Create a test signed message to demonstrate signature verification
            let test_msg = P2PMessage::Sync(SyncMessage::Put {
                key: "test".to_string(),
                value: "verification".to_string(),
                timestamp: chrono::Utc::now(),
            });

            match SignedData::new(test_msg, local_key) {
                Ok(signed_data) => match signed_data.verify(local_key) {
                    Ok(true) => {
                        println!("✓ Signature verification functionality working correctly")
                    }
                    Ok(false) => println!("✗ Signature verification failed"),
                    Err(e) => println!("✗ Signature verification error: {e}"),
                },
                Err(e) => println!("✗ Failed to create signed data: {e}"),
            }
        }
        ["test-access-control"] => {
            let test_config = SecurityConfig::default();
            let _test_access_control = AccessControl::new(test_config);
            println!("✓ Access control test completed");
        }
        _ => {
            println!("Unknown command: '{}'", input.trim());
            println!("Available commands: add, get, delete, list, status, peers, info, help");
            println!("Key distribution: announce-key, request-keys, request-whitelist");
            println!("Trust management: recommend-peer <peer_id>");
            println!("Maintenance: cleanup, reload-cache");
            println!("Type 'help' for detailed usage information.");
        }
    }

    Ok(())
}

async fn handle_swarm_event(
    swarm: &mut libp2p::Swarm<P2PSyncBehaviour>,
    storage: &Storage,
    topic: &gossipsub::IdentTopic,
    event: libp2p::swarm::SwarmEvent<
        <P2PSyncBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
    >,
    rate_limiter: &RateLimiter,
    connection_manager: &ConnectionManager,
    whitelist: &Arc<PeerWhitelist>,
    key_dist_manager: &Arc<KeyDistributionManager>,
) -> Result<()> {
    use libp2p::swarm::SwarmEvent;
    use tracing::warn;

    match event {
        SwarmEvent::Behaviour(behaviour_event) => {
            handle_behaviour_event(
                swarm,
                storage,
                topic,
                behaviour_event,
                rate_limiter,
                connection_manager,
                whitelist,
                key_dist_manager,
            )
            .await?;
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            info!("Local node is listening on {address}");
        }
        SwarmEvent::IncomingConnection { local_addr, .. } => {
            info!("Incoming connection from {local_addr}");
            // Note: We cannot extract peer_id here as it's not available in IncomingConnection
            // Actual connection tracking happens in ConnectionEstablished
        }
        SwarmEvent::ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            info!("Connection established with peer: {peer_id}");
            // Extract IP address from endpoint and handle connection
            if let Some(ip) =
                endpoint
                    .get_remote_address()
                    .iter()
                    .find_map(|protocol| match protocol {
                        libp2p::multiaddr::Protocol::Ip4(addr) => Some(std::net::IpAddr::V4(addr)),
                        libp2p::multiaddr::Protocol::Ip6(addr) => Some(std::net::IpAddr::V6(addr)),
                        _ => None,
                    })
            {
                if let Err(e) = connection_manager
                    .handle_incoming_connection(peer_id, ip)
                    .await
                {
                    tracing::warn!("Failed to handle incoming connection: {}", e);
                }
            }
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            warn!("Connection closed with peer {peer_id}: {cause:?}");
            connection_manager.handle_connection_closed(&peer_id).await;
        }
        _ => {}
    }

    Ok(())
}

async fn handle_behaviour_event(
    swarm: &mut libp2p::Swarm<P2PSyncBehaviour>,
    storage: &Storage,
    topic: &gossipsub::IdentTopic,
    event: <P2PSyncBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
    rate_limiter: &RateLimiter,
    connection_manager: &ConnectionManager,
    whitelist: &Arc<PeerWhitelist>,
    key_dist_manager: &Arc<KeyDistributionManager>,
) -> Result<()> {
    match event {
        network::P2PSyncBehaviourEvent::Mdns(mdns_event) => {
            handle_mdns_event(swarm, mdns_event).await?;
        }
        network::P2PSyncBehaviourEvent::Gossipsub(gossipsub_event) => {
            handle_gossipsub_event(
                storage,
                gossipsub_event,
                rate_limiter,
                connection_manager,
                whitelist,
                key_dist_manager,
                swarm,
                topic,
            )
            .await?;
        }
        network::P2PSyncBehaviourEvent::Kad(kad_event) => {
            info!("Kademlia event: {kad_event:?}");
        }
        network::P2PSyncBehaviourEvent::Identify(identify_event) => {
            info!("Identify event: {identify_event:?}");
        }
    }

    Ok(())
}

async fn handle_mdns_event(
    swarm: &mut libp2p::Swarm<P2PSyncBehaviour>,
    event: mdns::Event,
) -> Result<()> {
    match event {
        mdns::Event::Discovered(list) => {
            for (peer_id, addr) in list {
                info!("mDNS discovered a new peer: {peer_id}");
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }
        mdns::Event::Expired(list) => {
            for (peer_id, _) in list {
                info!("mDNS discover peer expired: {peer_id}");
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
            }
        }
    }

    Ok(())
}

async fn handle_gossipsub_event(
    storage: &Storage,
    event: gossipsub::Event,
    rate_limiter: &RateLimiter,
    connection_manager: &ConnectionManager,
    whitelist: &Arc<PeerWhitelist>,
    key_dist_manager: &Arc<KeyDistributionManager>,
    swarm: &mut libp2p::Swarm<P2PSyncBehaviour>,
    topic: &gossipsub::IdentTopic,
) -> Result<()> {
    use tracing::warn;

    match event {
        gossipsub::Event::Message {
            propagation_source: peer_id,
            message,
            ..
        } => {
            // レート制限チェック
            if let Err(e) = rate_limiter.check_rate_limit(&peer_id).await {
                warn!("Rate limit exceeded for peer {}: {}", peer_id, e);
                return Ok(());
            }

            // 接続状況チェック
            let active_connections = connection_manager.get_active_connections().await;
            if !active_connections.contains_key(&peer_id) {
                warn!("Message from unknown peer: {}", peer_id);
                return Ok(());
            }

            // メッセージサイズチェック
            if message.data.len() > 1024 * 1024 {
                // 1MB
                warn!(
                    "Message too large from peer {}: {} bytes",
                    peer_id,
                    message.data.len()
                );
                return Ok(());
            }

            // Parse signed P2P message
            let signed_data: SignedData<P2PMessage> = match serde_json::from_slice(&message.data) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Invalid signed message from peer {}: {}", peer_id, e);
                    return Ok(());
                }
            };

            // Verify sender's signature
            let signer_peer_id = match signed_data.signer.parse::<libp2p::PeerId>() {
                Ok(id) => id,
                Err(e) => {
                    warn!("Invalid signer peer ID from {}: {}", peer_id, e);
                    return Ok(());
                }
            };

            // Check if signer is whitelisted or trusted through recommendations
            if !whitelist.is_trusted_by_chain(&signer_peer_id).await? {
                warn!("Message from non-whitelisted peer: {}", signer_peer_id);
                return Ok(());
            }

            // Verify signature if public key is available
            if let Some(public_key) = whitelist.get_public_key(&signer_peer_id).await? {
                if !signed_data.verify_with_public_key(&public_key)? {
                    warn!("Invalid signature from peer: {}", signer_peer_id);
                    return Ok(());
                }
                info!("Signature verified for peer: {}", signer_peer_id);
            } else {
                // For peers without stored public keys, we trust based on whitelist only
                info!(
                    "No public key stored for peer {}, trusting based on whitelist",
                    signer_peer_id
                );
            }

            match signed_data.data {
                P2PMessage::Sync(sync_msg) => {
                    info!("Got sync message from {}: {:?}", signer_peer_id, sync_msg);

                    match sync_msg {
                        SyncMessage::Put {
                            key,
                            value,
                            timestamp,
                        } => {
                            // 入力検証
                            if let Err(e) = validate_key(&key, 256) {
                                warn!("Invalid key from peer {}: {}", peer_id, e);
                                return Ok(());
                            }
                            if let Err(e) = validate_value(&value, 64 * 1024) {
                                warn!("Invalid value from peer {}: {}", peer_id, e);
                                return Ok(());
                            }

                            storage.put_with_timestamp(&key, &value, timestamp)?;
                        }
                        SyncMessage::Delete { key, timestamp } => {
                            if let Err(e) = validate_key(&key, 256) {
                                warn!("Invalid key from peer {}: {}", peer_id, e);
                                return Ok(());
                            }

                            storage.delete_with_timestamp(&key, timestamp)?;
                        }
                    }
                }
                P2PMessage::KeyDistribution(key_msg) => {
                    info!(
                        "Got key distribution message from {}: {:?}",
                        signer_peer_id, key_msg
                    );

                    // Create a new SignedData for just the key distribution message
                    let key_signed_data = SignedData {
                        data: key_msg,
                        signature: signed_data.signature,
                        signer: signed_data.signer,
                    };

                    // Handle key distribution message
                    if let Some(response) = key_dist_manager
                        .handle_message(key_signed_data, signer_peer_id)
                        .await?
                    {
                        // Send response if needed
                        let p2p_response = P2PMessage::KeyDistribution(response);
                        let response_signed =
                            SignedData::new(p2p_response, key_dist_manager.local_keypair())?;

                        let response_json = serde_json::to_vec(&response_signed)?;
                        if response_json.len() <= 1024 * 1024 {
                            swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(topic.clone(), response_json)?;
                            info!("Sent key distribution response to {}", signer_peer_id);
                        }
                    }
                }
            }
        }
        gossipsub::Event::Subscribed { peer_id, topic } => {
            info!("Peer {peer_id} subscribed to topic: {topic}");
        }
        gossipsub::Event::Unsubscribed { peer_id, topic } => {
            info!("Peer {peer_id} unsubscribed from topic: {topic}");
        }
        _ => {}
    }

    Ok(())
}

fn install_service() -> Result<()> {
    autostart::setup_autostart()?;
    info!("Autostart service installed successfully");
    Ok(())
}

async fn handle_whitelist_command(cmd: WhitelistCommands) -> Result<()> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("p2p-sync");

    std::fs::create_dir_all(&data_dir)?;
    let whitelist = PeerWhitelist::new(&data_dir.join("whitelist.db"))?;

    match cmd {
        WhitelistCommands::Add {
            peer_id,
            name,
            expires_in_hours,
            public_key_file,
        } => {
            let peer_id = peer_id.parse::<libp2p::PeerId>()?;
            let expires_at = expires_in_hours
                .map(|hours| chrono::Utc::now() + chrono::Duration::hours(hours as i64));

            let public_key = if let Some(path) = public_key_file {
                match load_public_key_from_file(&path) {
                    Ok(pk) => Some(pk),
                    Err(e) => {
                        eprintln!("Warning: Failed to load public key from {path}: {e}");
                        None
                    }
                }
            } else {
                None
            };

            whitelist
                .add_peer(&peer_id, name, public_key.as_ref(), expires_at)
                .await?;

            if public_key.is_some() {
                println!("Added peer {peer_id} to whitelist with public key");
            } else {
                println!("Added peer {peer_id} to whitelist (no public key)");
            }
        }

        WhitelistCommands::Remove { peer_id } => {
            let peer_id = peer_id.parse::<libp2p::PeerId>()?;
            whitelist.remove_peer(&peer_id).await?;
            println!("Removed peer {peer_id} from whitelist");
        }

        WhitelistCommands::List => {
            let entries = whitelist.list_peers().await?;

            if entries.is_empty() {
                println!("No peers in whitelist");
            } else {
                println!("=== Whitelist Entries ===");
                println!(
                    "{:<60} {:<20} {:<20} {:<10}",
                    "Peer ID", "Name", "Expires", "Has Key"
                );
                println!("{}", "-".repeat(110));

                for entry in entries {
                    let expires = entry
                        .expires_at
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Never".to_string());

                    let has_key = if entry.public_key.is_some() {
                        "Yes"
                    } else {
                        "No"
                    };

                    println!(
                        "{:<60} {:<20} {:<20} {:<10}",
                        entry.peer_id,
                        entry.name.unwrap_or_else(|| "-".to_string()),
                        expires,
                        has_key
                    );
                }
            }
        }

        WhitelistCommands::Check { peer_id } => {
            let peer_id = peer_id.parse::<libp2p::PeerId>()?;
            let is_whitelisted = whitelist.is_whitelisted(&peer_id).await?;

            if is_whitelisted {
                println!("Peer {peer_id} is whitelisted");
            } else {
                println!("Peer {peer_id} is NOT whitelisted");
            }
        }

        WhitelistCommands::AddKey {
            peer_id,
            public_key_file,
        } => {
            let peer_id = peer_id.parse::<libp2p::PeerId>()?;

            // Check if peer is already in whitelist
            if !whitelist.is_whitelisted(&peer_id).await? {
                println!("Error: Peer {peer_id} is not in whitelist. Add the peer first.");
                return Ok(());
            }

            let public_key = load_public_key_from_file(&public_key_file)?;

            // Get existing entry details
            let entries = whitelist.list_peers().await?;
            let entry = entries.iter().find(|e| e.peer_id == peer_id.to_string());

            if let Some(entry) = entry {
                whitelist
                    .add_peer(
                        &peer_id,
                        entry.name.clone(),
                        Some(&public_key),
                        entry.expires_at,
                    )
                    .await?;
                println!("Updated public key for peer {peer_id}");
            } else {
                println!("Error: Peer {peer_id} not found in whitelist");
            }
        }
    }

    Ok(())
}

fn load_public_key_from_file(path: &str) -> Result<libp2p::identity::PublicKey> {
    use std::fs;

    let data = fs::read(path)?;

    // Try different formats
    // First try raw protobuf
    if let Ok(pk) = libp2p::identity::PublicKey::try_decode_protobuf(&data) {
        return Ok(pk);
    }

    // Try base64 decode then protobuf
    use base64::Engine;
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&data) {
        if let Ok(pk) = libp2p::identity::PublicKey::try_decode_protobuf(&decoded) {
            return Ok(pk);
        }
    }

    // Try as hex string
    let hex_str = String::from_utf8_lossy(&data).trim().to_string();
    if let Ok(decoded) = hex::decode(&hex_str) {
        if let Ok(pk) = libp2p::identity::PublicKey::try_decode_protobuf(&decoded) {
            return Ok(pk);
        }
    }

    anyhow::bail!("Unable to parse public key from file: {}", path);
}

use anyhow::Result;
use clap::{Parser, Subcommand};
use futures::StreamExt;
use libp2p::{gossipsub, identify, kad, mdns, noise, tcp, yamux, Multiaddr, SwarmBuilder};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tracing::info;

mod autostart;
mod config;
mod connection_manager;
mod network;
mod security;
mod storage;
mod sync;

use connection_manager::ConnectionManager;
use network::P2PSyncBehaviour;
use security::{validate_key, validate_value, sanitize_input, AccessControl, RateLimiter, SecurityConfig};
use storage::Storage;
use sync::SyncMessage;

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
    let access_control = AccessControl::new(config.security.clone());
    let connection_manager = ConnectionManager::new(access_control);

    let mut swarm = SwarmBuilder::with_new_identity()
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
                    handle_input(&mut swarm, &storage, &topic, line, &config.security, &connection_manager).await?;
                    // 次のプロンプトを表示
                    print!("> ");
                    std::io::stdout().flush()?;
                }
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, &storage, &topic, event, &rate_limiter, &connection_manager).await?;
            }
        }
    }
}

async fn handle_input(
    swarm: &mut libp2p::Swarm<P2PSyncBehaviour>,
    storage: &Storage,
    topic: &gossipsub::IdentTopic,
    input: String,
    security_config: &SecurityConfig,
    connection_manager: &ConnectionManager,
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

            let msg = SyncMessage::Put {
                key: sanitized_key.clone(),
                value: sanitized_value.clone(),
                timestamp: chrono::Utc::now(),
            };

            storage.put(&sanitized_key, &sanitized_value)?;

            let json = serde_json::to_vec(&msg)?;

            // メッセージサイズチェック
            if json.len() > security_config.max_message_size {
                anyhow::bail!("Message too large: {} bytes", json.len());
            }

            swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), json)?;

            println!("✓ Added: {} = {}", sanitized_key, sanitized_value);
            info!("Published: {} = {}", sanitized_key, sanitized_value);
        }
        ["get", key] => match storage.get(key)? {
            Some(value) => {
                println!("✓ {} = {}", key, value);
                info!("{} = {}", key, value);
            }
            None => {
                println!("✗ {} not found", key);
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
                    println!("  {} = {}", key, value);
                }
            }
        }
        ["status"] => {
            let connection_count = connection_manager.get_connection_count().await;
            let active_connections = connection_manager.get_active_connections().await;
            println!("=== P2P Status ===");
            println!("Active connections: {}", connection_count);
            if connection_count > 0 {
                println!("Connected peers:");
                for (peer_id, ip) in active_connections {
                    println!("  {} <- {}", peer_id, ip);
                }
            } else {
                println!("No active connections - waiting for peers...");
            }
            info!("Status checked - {} active connections", connection_count);
        }
        ["delete", key] => {
            let msg = SyncMessage::Delete {
                key: key.to_string(),
                timestamp: chrono::Utc::now(),
            };

            storage.delete_with_timestamp(key, chrono::Utc::now())?;

            let json = serde_json::to_vec(&msg)?;

            // メッセージサイズチェック
            if json.len() > security_config.max_message_size {
                anyhow::bail!("Message too large: {} bytes", json.len());
            }

            swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), json)?;

            println!("✓ Deleted: {}", key);
            info!("Deleted: {}", key);
        }
        ["help"] | ["h"] => {
            println!("Available commands:");
            println!("  add <key> <value>  - Add or update a key-value pair");
            println!("  get <key>          - Retrieve value for a key");
            println!("  delete <key>       - Delete a key-value pair");
            println!("  list               - List all stored items");
            println!("  status             - Show connection status");
            println!("  help               - Show this help message");
        }
        ["peers"] => {
            let active_connections = connection_manager.get_active_connections().await;
            if active_connections.is_empty() {
                println!("No connected peers");
            } else {
                println!("Connected peers ({}):", active_connections.len());
                for (peer_id, ip) in active_connections {
                    println!("  {} from {}", peer_id, ip);
                }
            }
        }
        ["info"] => {
            let local_peer_id = swarm.local_peer_id();
            let listeners: Vec<_> = swarm.listeners().collect();
            println!("=== Node Information ===");
            println!("Local Peer ID: {}", local_peer_id);
            println!("Listening on:");
            for addr in listeners {
                println!("  {}", addr);
            }
            let connection_count = connection_manager.get_connection_count().await;
            println!("Active connections: {}", connection_count);
        }
        _ => {
            println!("Unknown command: '{}'", input.trim());
            println!("Available commands: add, get, delete, list, status, peers, info, help");
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
        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
            info!("Connection established with peer: {peer_id}");
            // Extract IP address from endpoint and handle connection
            if let Some(ip) = endpoint.get_remote_address().iter()
                .find_map(|protocol| match protocol {
                    libp2p::multiaddr::Protocol::Ip4(addr) => Some(std::net::IpAddr::V4(addr)),
                    libp2p::multiaddr::Protocol::Ip6(addr) => Some(std::net::IpAddr::V6(addr)),
                    _ => None,
                }) {
                if let Err(e) = connection_manager.handle_incoming_connection(peer_id, ip).await {
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
    _topic: &gossipsub::IdentTopic,
    event: <P2PSyncBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
    rate_limiter: &RateLimiter,
    connection_manager: &ConnectionManager,
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
                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
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
            if message.data.len() > 1024 * 1024 { // 1MB
                warn!("Message too large from peer {}: {} bytes", peer_id, message.data.len());
                return Ok(());
            }
            
            let msg: SyncMessage = match serde_json::from_slice(&message.data) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Invalid message from peer {}: {}", peer_id, e);
                    return Ok(());
                }
            };
            
            info!("Got message from {peer_id}: {msg:?}");
            
            match msg {
                SyncMessage::Put { key, value, timestamp } => {
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

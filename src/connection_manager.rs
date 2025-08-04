use anyhow::Result;
use libp2p::PeerId;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::security::AccessControl;

type ActiveConnections = Arc<RwLock<HashMap<PeerId, IpAddr>>>;

pub struct ConnectionManager {
    access_control: Arc<AccessControl>,
    active_connections: ActiveConnections,
}

impl ConnectionManager {
    pub fn new(access_control: AccessControl) -> Self {
        Self {
            access_control: Arc::new(access_control),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn handle_incoming_connection(
        &self,
        peer_id: PeerId,
        remote_addr: IpAddr,
    ) -> Result<()> {
        // IP制限チェック
        self.access_control
            .check_connection_limit(&remote_addr)
            .await?;

        // ピア許可チェック
        self.access_control.check_peer_allowed(&peer_id).await?;

        // 接続を記録
        let mut connections = self.active_connections.write().await;
        connections.insert(peer_id, remote_addr);

        tracing::info!(
            "Connection accepted from peer: {} ({})",
            peer_id,
            remote_addr
        );
        Ok(())
    }

    pub async fn handle_connection_closed(&self, peer_id: &PeerId) {
        let mut connections = self.active_connections.write().await;
        if let Some(ip) = connections.remove(peer_id) {
            self.access_control.release_connection(&ip).await;
            tracing::info!("Connection closed for peer: {} ({})", peer_id, ip);
        }
    }

    pub async fn get_active_connections(&self) -> HashMap<PeerId, IpAddr> {
        self.active_connections.read().await.clone()
    }

    pub async fn get_connection_count(&self) -> usize {
        self.active_connections.read().await.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityConfig;
    use std::net::Ipv4Addr;

    fn create_test_connection_manager() -> ConnectionManager {
        let security_config = SecurityConfig::default();
        let access_control = AccessControl::new(security_config);
        ConnectionManager::new(access_control)
    }

    fn create_test_peer_id() -> PeerId {
        PeerId::random()
    }

    #[tokio::test]
    async fn test_new_connection_manager() {
        let manager = create_test_connection_manager();
        assert_eq!(manager.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_incoming_connection_success() {
        let manager = create_test_connection_manager();
        let peer_id = create_test_peer_id();
        let remote_addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        let result = manager
            .handle_incoming_connection(peer_id, remote_addr)
            .await;
        assert!(result.is_ok());
        assert_eq!(manager.get_connection_count().await, 1);

        let connections = manager.get_active_connections().await;
        assert_eq!(connections.get(&peer_id), Some(&remote_addr));
    }

    #[tokio::test]
    async fn test_handle_connection_closed() {
        let manager = create_test_connection_manager();
        let peer_id = create_test_peer_id();
        let remote_addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // First add a connection
        manager
            .handle_incoming_connection(peer_id, remote_addr)
            .await
            .unwrap();
        assert_eq!(manager.get_connection_count().await, 1);

        // Then close it
        manager.handle_connection_closed(&peer_id).await;
        assert_eq!(manager.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_connection_closed_nonexistent() {
        let manager = create_test_connection_manager();
        let peer_id = create_test_peer_id();

        // Close a connection that doesn't exist
        manager.handle_connection_closed(&peer_id).await;
        assert_eq!(manager.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_multiple_connections() {
        let manager = create_test_connection_manager();
        let mut peer_ids = Vec::new();
        let mut addrs = Vec::new();

        // Add multiple connections
        for i in 0..5 {
            let peer_id = create_test_peer_id();
            let addr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));

            manager
                .handle_incoming_connection(peer_id, addr)
                .await
                .unwrap();
            peer_ids.push(peer_id);
            addrs.push(addr);
        }

        assert_eq!(manager.get_connection_count().await, 5);

        let connections = manager.get_active_connections().await;
        for (peer_id, addr) in peer_ids.iter().zip(addrs.iter()) {
            assert_eq!(connections.get(peer_id), Some(addr));
        }
    }

    #[tokio::test]
    async fn test_connection_limit() {
        let security_config = SecurityConfig {
            max_connections_per_ip: 2,
            ..Default::default()
        };
        let access_control = AccessControl::new(security_config);
        let manager = ConnectionManager::new(access_control);

        let remote_addr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // First two connections should succeed
        let peer1 = create_test_peer_id();
        let peer2 = create_test_peer_id();

        assert!(manager
            .handle_incoming_connection(peer1, remote_addr)
            .await
            .is_ok());
        assert!(manager
            .handle_incoming_connection(peer2, remote_addr)
            .await
            .is_ok());

        // Third connection should fail
        let peer3 = create_test_peer_id();
        assert!(manager
            .handle_incoming_connection(peer3, remote_addr)
            .await
            .is_err());

        // After closing one, a new connection should succeed
        manager.handle_connection_closed(&peer1).await;
        assert!(manager
            .handle_incoming_connection(peer3, remote_addr)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let manager = Arc::new(create_test_connection_manager());
        let mut handles = vec![];

        // Spawn multiple concurrent operations
        for i in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                let peer_id = create_test_peer_id();
                let addr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, i));

                manager_clone
                    .handle_incoming_connection(peer_id, addr)
                    .await
                    .unwrap();
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                manager_clone.handle_connection_closed(&peer_id).await;
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // All connections should be closed
        assert_eq!(manager.get_connection_count().await, 0);
    }
}

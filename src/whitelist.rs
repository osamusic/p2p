use anyhow::Result;
use libp2p::PeerId;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub peer_id: String,
    pub name: Option<String>,
    pub public_key: Option<Vec<u8>>,
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,

    // Simple trust chain fields
    pub recommended_by: Vec<String>, // Peer IDs that recommended this peer
    pub recommendation_count: u32,   // Total number of recommendations received
}

pub struct PeerWhitelist {
    db: Arc<RwLock<Connection>>,
    cache: Arc<RwLock<HashSet<PeerId>>>,
}

impl PeerWhitelist {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(db_path: &Path) -> Result<Self> {
        let db = Connection::open(db_path)?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS peer_whitelist (
                peer_id TEXT PRIMARY KEY,
                name TEXT,
                public_key BLOB,
                added_at TEXT NOT NULL,
                expires_at TEXT,
                recommended_by TEXT DEFAULT '[]',
                recommendation_count INTEGER DEFAULT 0
            )",
            [],
        )?;

        // Add new columns if they don't exist (for existing databases)
        let _ = db.execute(
            "ALTER TABLE peer_whitelist ADD COLUMN recommended_by TEXT DEFAULT '[]'",
            [],
        );
        let _ = db.execute(
            "ALTER TABLE peer_whitelist ADD COLUMN recommendation_count INTEGER DEFAULT 0",
            [],
        );

        let whitelist = Self {
            db: Arc::new(RwLock::new(db)),
            cache: Arc::new(RwLock::new(HashSet::new())),
        };

        Ok(whitelist)
    }

    pub async fn add_peer(
        &self,
        peer_id: &PeerId,
        name: Option<String>,
        public_key: Option<&libp2p::identity::PublicKey>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        let peer_id_str = peer_id.to_string();
        let added_at = chrono::Utc::now();
        let public_key_bytes = public_key.map(|pk| pk.encode_protobuf());

        let db = self.db.write().await;
        db.execute(
            "INSERT OR REPLACE INTO peer_whitelist (peer_id, name, public_key, added_at, expires_at, recommended_by, recommendation_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                peer_id_str,
                name,
                public_key_bytes,
                added_at.to_rfc3339(),
                expires_at.map(|dt| dt.to_rfc3339()),
                "[]",  // Empty JSON array for recommended_by
                0      // Initial recommendation_count
            ],
        )?;

        let mut cache = self.cache.write().await;
        cache.insert(*peer_id);

        Ok(())
    }

    pub async fn remove_peer(&self, peer_id: &PeerId) -> Result<()> {
        let peer_id_str = peer_id.to_string();

        let db = self.db.write().await;
        db.execute(
            "DELETE FROM peer_whitelist WHERE peer_id = ?1",
            params![peer_id_str],
        )?;

        let mut cache = self.cache.write().await;
        cache.remove(peer_id);

        Ok(())
    }

    pub async fn is_whitelisted(&self, peer_id: &PeerId) -> Result<bool> {
        // まずキャッシュをチェック
        {
            let cache = self.cache.read().await;
            if cache.contains(peer_id) {
                // 有効期限をチェック
                let expires_at = {
                    let db = self.db.read().await;
                    let mut stmt =
                        db.prepare("SELECT expires_at FROM peer_whitelist WHERE peer_id = ?1")?;

                    stmt.query_row(params![peer_id.to_string()], |row| {
                        row.get::<_, Option<String>>(0)
                    })
                    .ok()
                    .flatten()
                };

                if let Some(expires_str) = expires_at {
                    if let Ok(expires_dt) = chrono::DateTime::parse_from_rfc3339(&expires_str) {
                        if expires_dt < chrono::Utc::now() {
                            // 期限切れなので削除（lockを先に解放）
                            drop(cache);
                            self.remove_peer(peer_id).await?;
                            return Ok(false);
                        }
                    }
                }

                return Ok(true);
            }
        }

        // キャッシュにない場合はDBをチェック
        let db = self.db.read().await;
        let mut stmt = db.prepare("SELECT expires_at FROM peer_whitelist WHERE peer_id = ?1")?;

        let result = stmt.query_row(params![peer_id.to_string()], |row| {
            let expires_at: Option<String> = row.get(0)?;
            Ok(expires_at)
        });

        match result {
            Ok(expires_at) => {
                if let Some(expires_str) = expires_at {
                    if let Ok(expires_dt) = chrono::DateTime::parse_from_rfc3339(&expires_str) {
                        if expires_dt < chrono::Utc::now() {
                            // 期限切れ
                            return Ok(false);
                        }
                    }
                }

                // キャッシュに追加
                let mut cache = self.cache.write().await;
                cache.insert(*peer_id);

                Ok(true)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn list_peers(&self) -> Result<Vec<WhitelistEntry>> {
        let db = self.db.read().await;
        let mut stmt = db.prepare(
            "SELECT peer_id, name, public_key, added_at, expires_at, recommended_by, recommendation_count FROM peer_whitelist ORDER BY added_at DESC"
        )?;

        let entries = stmt
            .query_map([], |row| {
                let peer_id: String = row.get(0)?;
                let name: Option<String> = row.get(1)?;
                let public_key: Option<Vec<u8>> = row.get(2)?;
                let added_at_str: String = row.get(3)?;
                let expires_at_str: Option<String> = row.get(4)?;
                let recommended_by_json: String = row.get(5).unwrap_or_else(|_| "[]".to_string());
                let recommendation_count: u32 = row.get(6).unwrap_or(0);

                let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                let expires_at = expires_at_str
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                let recommended_by: Vec<String> =
                    serde_json::from_str(&recommended_by_json).unwrap_or_else(|_| Vec::new());

                Ok(WhitelistEntry {
                    peer_id,
                    name,
                    public_key,
                    added_at,
                    expires_at,
                    recommended_by,
                    recommendation_count,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub async fn get_public_key(
        &self,
        peer_id: &PeerId,
    ) -> Result<Option<libp2p::identity::PublicKey>> {
        let db = self.db.read().await;
        let mut stmt = db.prepare("SELECT public_key FROM peer_whitelist WHERE peer_id = ?1")?;

        let result = stmt.query_row(params![peer_id.to_string()], |row| {
            let public_key_bytes: Option<Vec<u8>> = row.get(0)?;
            Ok(public_key_bytes)
        });

        match result {
            Ok(Some(bytes)) => match libp2p::identity::PublicKey::try_decode_protobuf(&bytes) {
                Ok(pk) => Ok(Some(pk)),
                Err(_) => Ok(None),
            },
            Ok(None) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn reload_cache(&self) -> Result<()> {
        let db = self.db.read().await;
        let mut stmt = db.prepare(
            "SELECT peer_id FROM peer_whitelist WHERE expires_at IS NULL OR expires_at > datetime('now')"
        )?;

        let peer_ids: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut cache = self.cache.write().await;
        cache.clear();

        for peer_id_str in peer_ids {
            if let Ok(peer_id) = peer_id_str.parse::<PeerId>() {
                cache.insert(peer_id);
            }
        }

        Ok(())
    }

    /// Check if a peer is trusted through direct whitelist or recommendations
    pub async fn is_trusted_by_chain(&self, peer_id: &PeerId) -> Result<bool> {
        // 1. Check if directly whitelisted
        if self.is_whitelisted(peer_id).await? {
            return Ok(true);
        }

        // 2. Check if recommended by whitelisted peers
        let entries = self.list_peers().await?;
        let peer_id_str = peer_id.to_string();

        for entry in entries {
            if entry.peer_id == peer_id_str && entry.recommendation_count > 0 {
                // Check if any recommender is still whitelisted
                for recommender_id_str in &entry.recommended_by {
                    if let Ok(recommender_peer_id) = recommender_id_str.parse::<libp2p::PeerId>() {
                        if self.is_whitelisted(&recommender_peer_id).await? {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Add a trust recommendation for a peer
    pub async fn add_recommendation(
        &self,
        peer_id: &PeerId,
        recommender_id: &PeerId,
        name: Option<String>,
    ) -> Result<()> {
        let peer_id_str = peer_id.to_string();
        let recommender_str = recommender_id.to_string();

        // Check if recommender is whitelisted
        if !self.is_whitelisted(recommender_id).await? {
            anyhow::bail!("Recommender {} is not whitelisted", recommender_str);
        }

        let db = self.db.write().await;

        // Get existing entry or create new one
        let mut stmt = db.prepare(
            "SELECT recommended_by, recommendation_count FROM peer_whitelist WHERE peer_id = ?1",
        )?;

        let existing = stmt.query_row([&peer_id_str], |row| {
            let recommended_by_json: String = row.get(0).unwrap_or_else(|_| "[]".to_string());
            let recommendation_count: u32 = row.get(1).unwrap_or(0);
            let recommended_by: Vec<String> =
                serde_json::from_str(&recommended_by_json).unwrap_or_else(|_| Vec::new());
            Ok((recommended_by, recommendation_count))
        });

        let (mut recommended_by, mut recommendation_count) = match existing {
            Ok((rec_by, rec_count)) => (rec_by, rec_count),
            Err(_) => (Vec::new(), 0),
        };

        // Add recommender if not already present
        if !recommended_by.contains(&recommender_str) {
            recommended_by.push(recommender_str);
            recommendation_count += 1;

            let recommended_by_json = serde_json::to_string(&recommended_by)?;

            db.execute(
                "INSERT OR REPLACE INTO peer_whitelist (peer_id, name, added_at, recommended_by, recommendation_count) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    peer_id_str,
                    name,
                    chrono::Utc::now().to_rfc3339(),
                    recommended_by_json,
                    recommendation_count
                ],
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_whitelist_add_remove() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("whitelist.db");
        let whitelist = PeerWhitelist::new(&db_path).unwrap();

        let peer_id = PeerId::random();

        // Add peer
        whitelist
            .add_peer(&peer_id, Some("Test Peer".to_string()), None, None)
            .await
            .unwrap();
        assert!(whitelist.is_whitelisted(&peer_id).await.unwrap());

        // Remove peer
        whitelist.remove_peer(&peer_id).await.unwrap();
        assert!(!whitelist.is_whitelisted(&peer_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_whitelist_expiration() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("whitelist.db");
        let whitelist = PeerWhitelist::new(&db_path).unwrap();

        let peer_id = PeerId::random();
        let expires_at = chrono::Utc::now() - chrono::Duration::hours(1); // Already expired

        whitelist
            .add_peer(&peer_id, None, None, Some(expires_at))
            .await
            .unwrap();
        assert!(!whitelist.is_whitelisted(&peer_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_whitelist_list() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("whitelist.db");
        let whitelist = PeerWhitelist::new(&db_path).unwrap();

        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        whitelist
            .add_peer(&peer1, Some("Peer 1".to_string()), None, None)
            .await
            .unwrap();
        whitelist
            .add_peer(&peer2, Some("Peer 2".to_string()), None, None)
            .await
            .unwrap();

        let entries = whitelist.list_peers().await.unwrap();
        assert_eq!(entries.len(), 2);
    }
}

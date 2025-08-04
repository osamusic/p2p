use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

type KeyValueList = Vec<(String, String)>;

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn put(&self, key: &str, value: &str) -> Result<()> {
        self.put_with_timestamp(key, value, Utc::now())
    }

    pub fn put_with_timestamp(
        &self,
        key: &str,
        value: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let existing_timestamp: Option<i64> = self
            .conn
            .query_row(
                "SELECT timestamp FROM kv_store WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(existing) = existing_timestamp {
            if existing > timestamp.timestamp() {
                return Ok(());
            }
        }

        self.conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, timestamp) VALUES (?1, ?2, ?3)",
            params![key, value, timestamp.timestamp()],
        )?;

        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .conn
            .query_row(
                "SELECT value FROM kv_store WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        Ok(value)
    }

    pub fn delete_with_timestamp(&self, key: &str, timestamp: DateTime<Utc>) -> Result<()> {
        let existing_timestamp: Option<i64> = self
            .conn
            .query_row(
                "SELECT timestamp FROM kv_store WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(existing) = existing_timestamp {
            if existing < timestamp.timestamp() {
                self.conn
                    .execute("DELETE FROM kv_store WHERE key = ?1", params![key])?;
            }
        }

        Ok(())
    }

    pub fn list(&self) -> Result<KeyValueList> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM kv_store ORDER BY key")?;

        let items = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use tempfile::tempdir;

    fn create_test_storage() -> (Storage, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Storage::new(&db_path).expect("Failed to create storage");
        (storage, temp_dir)
    }

    #[test]
    fn test_storage_creation() {
        let (storage, _dir) = create_test_storage();
        assert!(storage.list().is_ok());
    }

    #[test]
    fn test_storage_creation_invalid_path() {
        let result = Storage::new("/invalid/path/that/does/not/exist/test.db");
        assert!(result.is_err());
    }

    #[test]
    fn test_put_and_get() {
        let (storage, _dir) = create_test_storage();

        // Test basic put and get
        storage.put("key1", "value1").unwrap();
        let value = storage.get("key1").unwrap();
        assert_eq!(value, Some("value1".to_string()));
    }

    #[test]
    fn test_put_empty_key() {
        let (storage, _dir) = create_test_storage();

        // Empty key should be allowed
        storage.put("", "value").unwrap();
        let value = storage.get("").unwrap();
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_put_empty_value() {
        let (storage, _dir) = create_test_storage();

        // Empty value should be allowed
        storage.put("key", "").unwrap();
        let value = storage.get("key").unwrap();
        assert_eq!(value, Some("".to_string()));
    }

    #[test]
    fn test_put_unicode() {
        let (storage, _dir) = create_test_storage();

        // Test Unicode support
        storage.put("키", "값").unwrap();
        storage.put("🔑", "🎁").unwrap();

        assert_eq!(storage.get("키").unwrap(), Some("값".to_string()));
        assert_eq!(storage.get("🔑").unwrap(), Some("🎁".to_string()));
    }

    #[test]
    fn test_put_large_values() {
        let (storage, _dir) = create_test_storage();

        // Test large key and value
        let large_key = "k".repeat(1000);
        let large_value = "v".repeat(10000);

        storage.put(&large_key, &large_value).unwrap();
        let value = storage.get(&large_key).unwrap();
        assert_eq!(value, Some(large_value));
    }

    #[test]
    fn test_put_overwrite() {
        let (storage, _dir) = create_test_storage();

        // Test overwriting existing key
        storage.put("key", "value1").unwrap();
        storage.put("key", "value2").unwrap();

        let value = storage.get("key").unwrap();
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_put_with_timestamp_ordering() {
        let (storage, _dir) = create_test_storage();

        let early_time = Utc::now() - chrono::Duration::hours(1);
        let late_time = Utc::now();

        // Put with earlier timestamp
        storage
            .put_with_timestamp("key", "old_value", early_time)
            .unwrap();

        // Put with later timestamp should overwrite
        storage
            .put_with_timestamp("key", "new_value", late_time)
            .unwrap();

        let value = storage.get("key").unwrap();
        assert_eq!(value, Some("new_value".to_string()));
    }

    #[test]
    fn test_put_with_timestamp_ignore_old() {
        let (storage, _dir) = create_test_storage();

        let late_time = Utc::now();
        let early_time = Utc::now() - chrono::Duration::hours(1);

        // Put with later timestamp first
        storage
            .put_with_timestamp("key", "new_value", late_time)
            .unwrap();

        // Put with earlier timestamp should be ignored
        storage
            .put_with_timestamp("key", "old_value", early_time)
            .unwrap();

        let value = storage.get("key").unwrap();
        assert_eq!(value, Some("new_value".to_string()));
    }

    #[test]
    fn test_get_nonexistent_key() {
        let (storage, _dir) = create_test_storage();

        let value = storage.get("nonexistent").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_delete_with_timestamp() {
        let (storage, _dir) = create_test_storage();

        let put_time = Utc::now();
        let delete_time = Utc::now() + chrono::Duration::seconds(1);

        // Put a value
        storage
            .put_with_timestamp("key", "value", put_time)
            .unwrap();
        assert!(storage.get("key").unwrap().is_some());

        // Delete with later timestamp
        storage.delete_with_timestamp("key", delete_time).unwrap();
        assert!(storage.get("key").unwrap().is_none());
    }

    #[test]
    fn test_delete_with_timestamp_ignore_old() {
        let (storage, _dir) = create_test_storage();

        let put_time = Utc::now();
        let early_delete_time = Utc::now() - chrono::Duration::hours(1);

        // Put a value
        storage
            .put_with_timestamp("key", "value", put_time)
            .unwrap();

        // Delete with earlier timestamp should be ignored
        storage
            .delete_with_timestamp("key", early_delete_time)
            .unwrap();

        // Value should still exist
        assert_eq!(storage.get("key").unwrap(), Some("value".to_string()));
    }

    #[test]
    fn test_delete_nonexistent_key() {
        let (storage, _dir) = create_test_storage();

        // Delete non-existent key should not error
        storage
            .delete_with_timestamp("nonexistent", Utc::now())
            .unwrap();
    }

    #[test]
    fn test_list_empty() {
        let (storage, _dir) = create_test_storage();

        let items = storage.list().unwrap();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_list_multiple_items() {
        let (storage, _dir) = create_test_storage();

        storage.put("key1", "value1").unwrap();
        storage.put("key2", "value2").unwrap();
        storage.put("key3", "value3").unwrap();

        let items = storage.list().unwrap();
        assert_eq!(items.len(), 3);

        // Convert to HashMap for easier testing
        let items_map: std::collections::HashMap<_, _> = items.into_iter().collect();
        assert_eq!(items_map.get("key1"), Some(&"value1".to_string()));
        assert_eq!(items_map.get("key2"), Some(&"value2".to_string()));
        assert_eq!(items_map.get("key3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_list_after_delete() {
        let (storage, _dir) = create_test_storage();

        storage.put("key1", "value1").unwrap();
        storage.put("key2", "value2").unwrap();

        // Ensure delete happens after put by adding 1 second
        storage
            .delete_with_timestamp("key1", Utc::now() + chrono::Duration::seconds(1))
            .unwrap();

        let items = storage.list().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], ("key2".to_string(), "value2".to_string()));
    }

    #[test]
    fn test_sequential_operations() {
        let (storage, _dir) = create_test_storage();

        // Test sequential operations instead of concurrent
        for i in 0..10 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);

            storage.put(&key, &value).unwrap();
            let retrieved = storage.get(&key).unwrap();
            assert_eq!(retrieved, Some(value));
        }

        // Verify all items exist
        let items = storage.list().unwrap();
        assert_eq!(items.len(), 10);
    }

    #[test]
    fn test_special_characters_in_keys() {
        let (storage, _dir) = create_test_storage();

        let special_keys = vec![
            "key with spaces",
            "key\twith\ttabs",
            "key\nwith\nnewlines",
            "key'with'quotes",
            "key\"with\"double\"quotes",
            "key\\with\\backslashes",
            "key/with/slashes",
            "key:with:colons",
            "key;with;semicolons",
            "key|with|pipes",
        ];

        for key in &special_keys {
            storage.put(key, "value").unwrap();
            let value = storage.get(key).unwrap();
            assert_eq!(value, Some("value".to_string()));
        }

        let items = storage.list().unwrap();
        assert_eq!(items.len(), special_keys.len());
    }

    #[test]
    fn test_binary_data_as_strings() {
        let (storage, _dir) = create_test_storage();

        // Test storing binary-like data as strings
        let binary_like = "binary_data_00FF8040201008040201";

        storage.put("binary_key", binary_like).unwrap();
        let retrieved = storage.get("binary_key").unwrap();

        assert_eq!(retrieved, Some(binary_like.to_string()));
    }
}

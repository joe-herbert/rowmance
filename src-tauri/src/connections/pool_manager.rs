/// Manages the set of active remote database connection pools.
///
/// Each connection profile gets its own pool keyed by profile UUID.
/// DashMap provides lock-free concurrent reads, which is important because
/// many Tauri commands run concurrently on the Tokio runtime and all need
/// to look up the pool on every query.
use dashmap::DashMap;
use uuid;
use std::sync::Arc;

use crate::connections::engine::DatabaseEngine;
use crate::error::RowmanceError;

/// Opaque handle for an active connection pool.
/// The adapter inside handles all engine-specific lifecycle operations.
#[derive(Debug)]
pub struct RemotePool(Arc<dyn crate::connections::engine::PoolAdapter>);

/// Thread-safe registry of active remote pools.
#[derive(Debug, Default)]
pub struct ConnectionManager {
    pools: DashMap<String, RemotePool>,
    names: DashMap<String, String>,
}

impl ConnectionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pools: DashMap::new(),
            names: DashMap::new(),
        })
    }

    /// Pre-register a connection name so error messages use it even before the pool is opened.
    pub fn register_name(&self, id: &str, name: &str) {
        self.names.insert(id.to_owned(), name.to_owned());
    }

    /// Open a new pool for the given connection profile and register it.
    /// If a pool already exists for this id it is replaced.
    #[allow(clippy::too_many_arguments)]
    pub async fn connect(
        &self,
        id: &str,
        name: &str,
        db_type: &str,
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
        pool_max: u32,
        ssl_enabled: bool,
        ssl_ca_path: Option<&str>,
        ssl_cert_path: Option<&str>,
        ssl_key_path: Option<&str>,
        read_only: bool,
    ) -> Result<(), RowmanceError> {
        let adapter = crate::connections::engines::connect_for_db_type(
            db_type, host, port, database, username, password, pool_max,
            ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path, read_only,
        ).await?;
        self.pools.insert(id.to_owned(), RemotePool(Arc::from(adapter)));
        self.names.insert(id.to_owned(), name.to_owned());
        Ok(())
    }

    /// Test a connection without persisting a pool.
    /// Creates a temporary single-connection pool, verifies credentials, then tears it down.
    /// All engine-specific logic is delegated to `connect()` — no engine branching here.
    #[allow(clippy::too_many_arguments)]
    pub async fn test_connect(
        &self,
        db_type: &str,
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
        ssl_enabled: bool,
        ssl_ca_path: Option<&str>,
        ssl_cert_path: Option<&str>,
        ssl_key_path: Option<&str>,
    ) -> Result<(), crate::error::RowmanceError> {
        let temp_id = format!("__test__{}", uuid::Uuid::new_v4());
        self.connect(
            &temp_id,
            "__test__",
            db_type,
            host,
            port,
            database,
            username,
            password,
            1,
            ssl_enabled,
            ssl_ca_path,
            ssl_cert_path,
            ssl_key_path,
            false,
        )
        .await?;
        self.disconnect(&temp_id).await;
        Ok(())
    }

    /// Close and remove the pool for the given connection id.
    pub async fn disconnect(&self, id: &str) {
        if let Some((_, pool)) = self.pools.remove(id) {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                pool.0.disconnect(),
            ).await;
        }
    }

    /// Send a lightweight ping to check the connection is still alive.
    /// Returns false if no pool exists or the query fails.
    pub async fn ping(&self, id: &str) -> bool {
        let Ok(pool) = self.get(id) else { return false; };
        let adapter = pool.0.clone();
        drop(pool);
        adapter.ping().await
    }

    /// Returns true if a pool exists for the given id.
    pub fn is_active(&self, id: &str) -> bool {
        self.pools.contains_key(id)
    }

    /// Returns the ids of all currently active connections.
    pub fn active_ids(&self) -> Vec<String> {
        self.pools.iter().map(|e| e.key().clone()).collect()
    }

    /// Obtain a reference to the pool for a connection id.
    /// Returns an error if no pool is registered for that id.
    pub fn get(
        &self,
        id: &str,
    ) -> Result<dashmap::mapref::one::Ref<'_, String, RemotePool>, RowmanceError> {
        self.pools.get(id).ok_or_else(|| {
            let name = self
                .names
                .get(id)
                .map(|n| n.clone())
                .unwrap_or_else(|| id.to_owned());
            RowmanceError::ConnectionNotActive(name)
        })
    }

    /// Construct a `DatabaseEngine` trait object for the given connection id.
    /// Clones the underlying pool handle (cheap — pools are internally Arc-wrapped).
    pub fn get_engine(&self, id: &str) -> Result<Arc<dyn DatabaseEngine>, RowmanceError> {
        let pool_ref = self.get(id)?;
        Ok(pool_ref.0.get_engine())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_ids_empty_on_new() {
        let manager = ConnectionManager::new();
        assert!(manager.active_ids().is_empty());
        assert!(!manager.is_active("nonexistent"));
    }

    #[test]
    fn get_returns_error_for_unknown_id() {
        let manager = ConnectionManager::new();
        let err = manager.get("missing").unwrap_err();
        assert!(matches!(err, RowmanceError::ConnectionNotActive(_)));
    }

    #[test]
    fn is_active_returns_false_for_missing_id() {
        let manager = ConnectionManager::new();
        assert!(!manager.is_active("any-id"));
    }

    #[test]
    fn active_ids_reflects_current_registry() {
        // We can't insert a real pool without a running DB, but we can assert
        // the registry is consistent at construction time.
        let manager = ConnectionManager::new();
        assert_eq!(manager.active_ids().len(), 0);
    }

    #[tokio::test]
    async fn disconnect_nonexistent_id_is_a_noop() {
        let manager = ConnectionManager::new();
        // Should not panic.
        manager.disconnect("ghost").await;
        assert!(!manager.is_active("ghost"));
    }

    #[test]
    fn connect_with_unknown_db_type_returns_error() {
        // This test exercises the synchronous path check (no real network needed).
        // We use a block_on wrapper since ConnectionManager::connect is async.
        let manager = ConnectionManager::new();
        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
            manager
                .connect(
                    "id",
                    "name",
                    "oracle",
                    "localhost",
                    1521,
                    "db",
                    "user",
                    "pw",
                    1,
                    false,
                    None,
                    None,
                    None,
                    false,
                )
                .await
        });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RowmanceError::ConnectionNotFound(_)
        ));
    }

    #[test]
    fn unknown_db_type_errors_even_with_ssl_enabled() {
        let manager = ConnectionManager::new();
        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
            manager
                .connect(
                    "id",
                    "name",
                    "oracle",
                    "localhost",
                    1521,
                    "db",
                    "user",
                    "pw",
                    1,
                    true,
                    Some("/tmp/ca.pem"),
                    None,
                    None,
                    false,
                )
                .await
        });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RowmanceError::ConnectionNotFound(_)
        ));
    }
}

/// Manages the set of active remote database connection pools.
///
/// Each connection profile gets its own sqlx pool keyed by profile UUID.
/// DashMap provides lock-free concurrent reads, which is important because
/// many Tauri commands run concurrently on the Tokio runtime and all need
/// to look up the pool on every query.
use dashmap::DashMap;
use sqlx::{mysql::MySqlPoolOptions, postgres::PgPoolOptions};
use std::sync::Arc;

use crate::error::RowmanceError;

/// Unified handle for a pool that may be either MySQL/MariaDB or PostgreSQL.
#[derive(Debug)]
pub enum RemotePool {
    MySql(sqlx::MySqlPool),
    Postgres(sqlx::PgPool),
}

/// Thread-safe registry of active remote pools.
#[derive(Debug, Default)]
pub struct ConnectionManager {
    pools: DashMap<String, RemotePool>,
}

impl ConnectionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pools: DashMap::new(),
        })
    }

    /// Open a new pool for the given connection profile and register it.
    /// If a pool already exists for this id it is replaced.
    #[allow(clippy::too_many_arguments)]
    pub async fn connect(
        &self,
        id: &str,
        db_type: &str,
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
        pool_min: u32,
        pool_max: u32,
    ) -> Result<(), RowmanceError> {
        let pool = match db_type {
            "mysql" | "mariadb" => {
                let url = format!(
                    "mysql://{}:{}@{}:{}/{}",
                    username, password, host, port, database
                );
                let p = MySqlPoolOptions::new()
                    .min_connections(pool_min)
                    .max_connections(pool_max)
                    .connect(&url)
                    .await?;
                RemotePool::MySql(p)
            }
            "postgres" => {
                let url = format!(
                    "postgres://{}:{}@{}:{}/{}",
                    username, password, host, port, database
                );
                let p = PgPoolOptions::new()
                    .min_connections(pool_min)
                    .max_connections(pool_max)
                    .connect(&url)
                    .await?;
                RemotePool::Postgres(p)
            }
            other => {
                return Err(RowmanceError::ConnectionNotFound(format!(
                    "Unknown db_type: {other}"
                )));
            }
        };

        self.pools.insert(id.to_owned(), pool);
        Ok(())
    }

    /// Close and remove the pool for the given connection id.
    pub async fn disconnect(&self, id: &str) {
        if let Some((_, pool)) = self.pools.remove(id) {
            match pool {
                RemotePool::MySql(p) => p.close().await,
                RemotePool::Postgres(p) => p.close().await,
            }
        }
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
        self.pools
            .get(id)
            .ok_or_else(|| RowmanceError::ConnectionNotActive(id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_ids_empty_on_new() {
        let manager = ConnectionManager::new();
        assert!(manager.active_ids().is_empty());
        assert!(!manager.is_active("nonexistent"));
    }

    #[test]
    fn test_get_returns_error_for_unknown_id() {
        let manager = ConnectionManager::new();
        assert!(manager.get("missing").is_err());
    }
}

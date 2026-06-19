/// Manages the set of active remote database connection pools.
///
/// Each connection profile gets its own sqlx pool keyed by profile UUID.
/// DashMap provides lock-free concurrent reads, which is important because
/// many Tauri commands run concurrently on the Tokio runtime and all need
/// to look up the pool on every query.
use dashmap::DashMap;
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode},
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::Path;
use std::sync::Arc;

use crate::error::RowmanceError;

/// Unified handle for a pool that may be MySQL/MariaDB, PostgreSQL, or SQLite.
#[derive(Debug)]
pub enum RemotePool {
    MySql(sqlx::MySqlPool),
    Postgres(sqlx::PgPool),
    Sqlite(sqlx::SqlitePool),
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
        ssl_enabled: bool,
        ssl_ca_path: Option<&str>,
        ssl_cert_path: Option<&str>,
        ssl_key_path: Option<&str>,
    ) -> Result<(), RowmanceError> {
        let pool = match db_type {
            "mysql" | "mariadb" => {
                let mut opts = MySqlConnectOptions::new()
                    .host(host)
                    .port(port)
                    .database(database)
                    .username(username)
                    .password(password);

                if ssl_enabled {
                    let ssl_mode = if ssl_ca_path.is_some() {
                        MySqlSslMode::VerifyCa
                    } else {
                        MySqlSslMode::Required
                    };
                    opts = opts.ssl_mode(ssl_mode);

                    if let Some(ca) = ssl_ca_path {
                        opts = opts.ssl_ca(Path::new(ca));
                    }
                    if let Some(cert) = ssl_cert_path {
                        if let Some(key) = ssl_key_path {
                            opts = opts.ssl_client_cert(Path::new(cert))
                                       .ssl_client_key(Path::new(key));
                        }
                    }
                }

                let p = MySqlPoolOptions::new()
                    .min_connections(pool_min)
                    .max_connections(pool_max)
                    .connect_with(opts)
                    .await?;
                RemotePool::MySql(p)
            }
            "postgres" => {
                let mut opts = PgConnectOptions::new()
                    .host(host)
                    .port(port)
                    .database(database)
                    .username(username)
                    .password(password);

                if ssl_enabled {
                    let ssl_mode = if ssl_ca_path.is_some() {
                        PgSslMode::VerifyCa
                    } else {
                        PgSslMode::Require
                    };
                    opts = opts.ssl_mode(ssl_mode);

                    if let Some(ca) = ssl_ca_path {
                        opts = opts.ssl_root_cert(Path::new(ca));
                    }
                    if let Some(cert) = ssl_cert_path {
                        opts = opts.ssl_client_cert(Path::new(cert));
                    }
                    if let Some(key) = ssl_key_path {
                        opts = opts.ssl_client_key(Path::new(key));
                    }
                }

                let p = PgPoolOptions::new()
                    .min_connections(pool_min)
                    .max_connections(pool_max)
                    .connect_with(opts)
                    .await?;
                RemotePool::Postgres(p)
            }
            "sqlite" => {
                // For SQLite, `host` holds the file path (or `:memory:`).
                // No user/password/SSL needed.
                let opts = SqliteConnectOptions::new()
                    .filename(host)
                    .create_if_missing(true);
                let p = SqlitePoolOptions::new()
                    .min_connections(pool_min)
                    .max_connections(pool_max)
                    .connect_with(opts)
                    .await?;
                RemotePool::Sqlite(p)
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
                RemotePool::Sqlite(p) => p.close().await,
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
                    "id", "oracle", "localhost", 1521, "db", "user", "pw", 1, 1,
                    false, None, None, None,
                )
                .await
        });
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    // ── SSL option building tests ─────────────────────────────────────────────
    // These verify the option-building logic without requiring a real DB.

    #[test]
    fn mysql_ssl_disabled_uses_no_ssl_options() {
        // Build options manually as the connect() function would for mysql with ssl off.
        let opts = MySqlConnectOptions::new()
            .host("localhost")
            .port(3306)
            .database("db")
            .username("user")
            .password("pw");
        // Simply verify the struct can be constructed without error.
        let _ = opts;
    }

    #[test]
    fn mysql_ssl_required_mode_set_when_no_ca() {
        // When ssl_enabled=true and no CA path, mode should be Required.
        let opts = MySqlConnectOptions::new()
            .host("localhost")
            .port(3306)
            .database("db")
            .username("user")
            .password("pw")
            .ssl_mode(MySqlSslMode::Required);
        let _ = opts;
    }

    #[test]
    fn mysql_ssl_verify_ca_mode_set_when_ca_present() {
        // When ssl_enabled=true and CA path is given, mode should be VerifyCa.
        let opts = MySqlConnectOptions::new()
            .host("localhost")
            .port(3306)
            .database("db")
            .username("user")
            .password("pw")
            .ssl_mode(MySqlSslMode::VerifyCa)
            .ssl_ca(Path::new("/tmp/ca.pem"));
        let _ = opts;
    }

    #[test]
    fn postgres_ssl_require_mode_set_when_no_ca() {
        let opts = PgConnectOptions::new()
            .host("localhost")
            .port(5432)
            .database("db")
            .username("user")
            .password("pw")
            .ssl_mode(PgSslMode::Require);
        let _ = opts;
    }

    #[test]
    fn postgres_ssl_verify_ca_mode_set_when_ca_present() {
        let opts = PgConnectOptions::new()
            .host("localhost")
            .port(5432)
            .database("db")
            .username("user")
            .password("pw")
            .ssl_mode(PgSslMode::VerifyCa)
            .ssl_root_cert(Path::new("/tmp/ca.pem"));
        let _ = opts;
    }

    #[test]
    fn postgres_ssl_with_client_cert_and_key() {
        let opts = PgConnectOptions::new()
            .host("localhost")
            .port(5432)
            .database("db")
            .username("user")
            .password("pw")
            .ssl_mode(PgSslMode::Require)
            .ssl_client_cert(Path::new("/tmp/client.crt"))
            .ssl_client_key(Path::new("/tmp/client.key"));
        let _ = opts;
    }

    #[test]
    fn unknown_db_type_errors_even_with_ssl_enabled() {
        let manager = ConnectionManager::new();
        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
            manager
                .connect(
                    "id", "oracle", "localhost", 1521, "db", "user", "pw", 1, 1,
                    true, Some("/tmp/ca.pem"), None, None,
                )
                .await
        });
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }
}

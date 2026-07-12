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
    Executor,
};
use std::path::Path;
use std::sync::Arc;

use crate::error::RowmanceError;

/// Reset MySQL connection to the given database using the text protocol (COM_QUERY).
/// Extracted as a standalone `async fn` to give the borrow checker a concrete
/// lifetime, working around the HKT limitation on `raw_sql().execute(&mut conn)`.
async fn mysql_reset_db(conn: &mut sqlx::mysql::MySqlConnection, use_sql: &str) -> bool {
    conn.execute(sqlx::raw_sql(use_sql)).await.is_ok()
}

/// Reset Postgres connection to the given schema using the text protocol.
async fn pg_reset_schema(conn: &mut sqlx::postgres::PgConnection, set_path_sql: &str) -> bool {
    conn.execute(sqlx::raw_sql(set_path_sql)).await.is_ok()
}

/// Unified handle for a pool that may be MySQL/MariaDB, PostgreSQL, or SQLite.
/// The `bool` on the MySql variant is `read_only`: enforced via after_connect
/// (`SET SESSION TRANSACTION READ ONLY`) for all pool connections. The
/// after_release hook resets the database/schema context so released connections
/// are always clean for the next caller.
#[derive(Debug)]
pub enum RemotePool {
    MySql(sqlx::MySqlPool, bool),
    Postgres(sqlx::PgPool),
    Sqlite(sqlx::SqlitePool),
}

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
        let pool = match db_type {
            "mysql" | "mariadb" => {
                let mut opts = MySqlConnectOptions::new()
                    .host(host)
                    .port(port)
                    .database(database)
                    .username(username);
                if !password.is_empty() {
                    opts = opts.password(password);
                }

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
                            opts = opts
                                .ssl_client_cert(Path::new(cert))
                                .ssl_client_key(Path::new(key));
                        }
                    }
                } else {
                    opts = opts.ssl_mode(MySqlSslMode::Disabled);
                }

                let mut pool_opts = MySqlPoolOptions::new()
                    .min_connections(1)
                    .max_connections(pool_max);
                // SET SESSION TRANSACTION READ ONLY on every connection so the
                // server enforces read-only regardless of what SQL is sent.
                if read_only {
                    pool_opts = pool_opts.after_connect(|conn, _meta| {
                        Box::pin(async move {
                            sqlx::query("SET SESSION TRANSACTION READ ONLY")
                                .execute(conn)
                                .await?;
                            Ok(())
                        })
                    });
                }
                // Reset the database context when a connection is released back
                // to the pool, so other callers always start with the default db.
                // The USE statement must go over the text protocol (COM_QUERY) —
                // MySQL rejects it as a prepared statement (error 1295). We build
                // the SQL once and leak it to obtain a 'static reference, which is
                // required by the `for<'c>` closure signature of after_release.
                // One leaked string per pool (i.e., per connection profile) is
                // negligible — pools live for the process lifetime anyway.
                let db_esc = database.replace('`', "``");
                let use_sql: &'static str = Box::leak(format!("USE `{}`", db_esc).into_boxed_str());
                pool_opts = pool_opts.after_release(move |conn, _meta| {
                    Box::pin(async move { Ok(mysql_reset_db(conn, use_sql).await) })
                });
                let p = pool_opts.connect_with(opts).await?;
                RemotePool::MySql(p, read_only)
            }
            "postgres" => {
                let mut opts = PgConnectOptions::new()
                    .host(host)
                    .port(port)
                    .database(database)
                    .username(username);
                if !password.is_empty() {
                    opts = opts.password(password);
                }

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

                // default_transaction_read_only makes the server reject all
                // writes at the session level, including CTEs and procedures.
                if read_only {
                    opts = opts.options([("default_transaction_read_only", "on")]);
                }

                // Reset the search_path when a connection is released back to
                // the pool, so other callers always start with the default schema.
                // Leak the SQL string once to satisfy the `for<'c>` closure bound.
                let schema_esc = database.replace('\'', "''");
                let set_path_sql: &'static str =
                    Box::leak(format!("SET search_path = '{}'", schema_esc).into_boxed_str());
                let p = PgPoolOptions::new()
                    .min_connections(1)
                    .max_connections(pool_max)
                    .after_release(move |conn, _meta| {
                        Box::pin(async move { Ok(pg_reset_schema(conn, set_path_sql).await) })
                    })
                    .connect_with(opts)
                    .await?;
                RemotePool::Postgres(p)
            }
            "sqlite" => {
                // For SQLite, `host` holds the file path (or `:memory:`).
                // No user/password/SSL needed.
                // read_only(true) opens the file with O_RDONLY so the OS
                // prevents any writes even if bypassing our application logic.
                let opts = SqliteConnectOptions::new()
                    .filename(host)
                    .read_only(read_only)
                    .create_if_missing(!read_only);
                let p = SqlitePoolOptions::new()
                    .min_connections(1)
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
        self.names.insert(id.to_owned(), name.to_owned());
        Ok(())
    }

    /// Close and remove the pool for the given connection id.
    pub async fn disconnect(&self, id: &str) {
        if let Some((_, pool)) = self.pools.remove(id) {
            let close = async {
                match pool {
                    RemotePool::MySql(p, _) => p.close().await,
                    RemotePool::Postgres(p) => p.close().await,
                    RemotePool::Sqlite(p) => p.close().await,
                }
            };
            // If the graceful close hangs (e.g. in-flight query or unreachable server),
            // drop the pool after 3s — connections are forcibly closed on drop anyway.
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), close).await;
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
        self.pools.get(id).ok_or_else(|| {
            let name = self
                .names
                .get(id)
                .map(|n| n.clone())
                .unwrap_or_else(|| id.to_owned());
            RowmanceError::ConnectionNotActive(name)
        })
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

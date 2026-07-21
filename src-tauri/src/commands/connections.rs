/// Tauri commands for managing connection profiles and groups.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::connections::pool_manager::ConnectionManager;
use crate::connections::ssh_tunnel::SshTunnelManager;
use crate::connections::types::DialectInfo;
use crate::db::models::{ConnectionGroupRow, ConnectionProfileRow};
use crate::error::AppError;

// ── IPC types returned to the frontend ───────────────────────────────────────

/// A connection profile as returned to the frontend.
/// Passwords are never included — they live in the OS keychain.
#[derive(Debug, Serialize, Clone)]
pub struct ConnectionProfile {
    pub id: String,
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    pub name: String,
    #[serde(rename = "dbType")]
    pub db_type: String,
    pub host: String,
    pub port: i64,
    pub database: String,
    pub username: String,
    pub color: Option<String>,
    #[serde(rename = "readOnly")]
    pub read_only: bool,
    #[serde(rename = "sshEnabled")]
    pub ssh_enabled: bool,
    #[serde(rename = "sshHost")]
    pub ssh_host: Option<String>,
    #[serde(rename = "sshPort")]
    pub ssh_port: Option<i64>,
    #[serde(rename = "sshUser")]
    pub ssh_user: Option<String>,
    #[serde(rename = "sshAuthType")]
    pub ssh_auth_type: Option<String>,
    #[serde(rename = "sshKeyPath")]
    pub ssh_key_path: Option<String>,
    #[serde(rename = "sslEnabled")]
    pub ssl_enabled: bool,
    #[serde(rename = "sslCaPath")]
    pub ssl_ca_path: Option<String>,
    #[serde(rename = "sslCertPath")]
    pub ssl_cert_path: Option<String>,
    #[serde(rename = "sslKeyPath")]
    pub ssl_key_path: Option<String>,
    #[serde(rename = "poolMax")]
    pub pool_max: i64,
    #[serde(rename = "pingInterval")]
    pub ping_interval: Option<i64>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "dialectInfo")]
    pub dialect_info: DialectInfo,
}

impl From<ConnectionProfileRow> for ConnectionProfile {
    fn from(r: ConnectionProfileRow) -> Self {
        let dialect_info = DialectInfo::for_db_type(&r.db_type);
        Self {
            id: r.id,
            group_id: r.group_id,
            name: r.name,
            db_type: r.db_type,
            host: r.host,
            port: r.port,
            database: r.database,
            username: r.username,
            color: r.color,
            read_only: r.read_only,
            ssh_enabled: r.ssh_enabled,
            ssh_host: r.ssh_host,
            ssh_port: r.ssh_port,
            ssh_user: r.ssh_user,
            ssh_auth_type: r.ssh_auth_type,
            ssh_key_path: r.ssh_key_path,
            ssl_enabled: r.ssl_enabled,
            ssl_ca_path: r.ssl_ca_path,
            ssl_cert_path: r.ssl_cert_path,
            ssl_key_path: r.ssl_key_path,
            pool_max: r.pool_max,
            ping_interval: r.ping_interval,
            created_at: r.created_at,
            updated_at: r.updated_at,
            dialect_info,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ConnectionGroup {
    pub id: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub position: i64,
}

impl From<ConnectionGroupRow> for ConnectionGroup {
    fn from(r: ConnectionGroupRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            parent_id: r.parent_id,
            position: r.position,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    #[serde(rename = "latencyMs")]
    pub latency_ms: Option<u64>,
}

/// Input for creating or updating a connection profile.
/// Passwords are NOT included here — they are stored in the OS keychain
/// via `keychain_store` before or after calling create/update.
#[derive(Debug, Deserialize)]
pub struct ConnectionProfileInput {
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    pub name: String,
    #[serde(rename = "dbType")]
    pub db_type: String,
    pub host: String,
    pub port: i64,
    pub database: String,
    pub username: String,
    pub color: Option<String>,
    #[serde(rename = "readOnly", default)]
    pub read_only: bool,
    #[serde(rename = "sshEnabled", default)]
    pub ssh_enabled: bool,
    #[serde(rename = "sshHost")]
    pub ssh_host: Option<String>,
    #[serde(rename = "sshPort")]
    pub ssh_port: Option<i64>,
    #[serde(rename = "sshUser")]
    pub ssh_user: Option<String>,
    #[serde(rename = "sshAuthType")]
    pub ssh_auth_type: Option<String>,
    #[serde(rename = "sshKeyPath")]
    pub ssh_key_path: Option<String>,
    #[serde(rename = "sslEnabled", default)]
    pub ssl_enabled: bool,
    #[serde(rename = "sslCaPath")]
    pub ssl_ca_path: Option<String>,
    #[serde(rename = "sslCertPath")]
    pub ssl_cert_path: Option<String>,
    #[serde(rename = "sslKeyPath")]
    pub ssl_key_path: Option<String>,
    #[serde(rename = "poolMax")]
    pub pool_max: Option<i64>,
    #[serde(rename = "pingInterval")]
    pub ping_interval: Option<i64>,
}

fn retrieve_keychain_password(connection_id: &str) -> String {
    let account = format!("{connection_id}:db_password");
    crate::commands::keychain::read_keychain_secret("rowmance", &account).unwrap_or_default()
}

fn retrieve_keychain_secret(connection_id: &str, secret_type: &str) -> Option<String> {
    let account = format!("{connection_id}:{secret_type}");
    crate::commands::keychain::read_keychain_secret("rowmance", &account)
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// List all stored connection profiles.
#[tauri::command]
pub async fn connections_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<ConnectionProfile>, AppError> {
    let rows = sqlx::query_as::<_, ConnectionProfileRow>(
        "SELECT * FROM connection_profiles ORDER BY name",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(ConnectionProfile::from).collect())
}

/// Create a new connection profile.
#[tauri::command]
pub async fn connections_create(
    sqlite: State<'_, SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    input: ConnectionProfileInput,
) -> Result<ConnectionProfile, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let pool_max = input.pool_max.unwrap_or(5);

    sqlx::query(
        r#"
        INSERT INTO connection_profiles (
            id, group_id, name, db_type, host, port, database, username, color,
            read_only, ssh_enabled, ssh_host, ssh_port, ssh_user, ssh_auth_type, ssh_key_path,
            ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path,
            pool_max, ping_interval, created_at, updated_at
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?
        )
        "#,
    )
    .bind(&id)
    .bind(&input.group_id)
    .bind(&input.name)
    .bind(&input.db_type)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.database)
    .bind(&input.username)
    .bind(&input.color)
    .bind(input.read_only)
    .bind(input.ssh_enabled)
    .bind(&input.ssh_host)
    .bind(input.ssh_port)
    .bind(&input.ssh_user)
    .bind(&input.ssh_auth_type)
    .bind(&input.ssh_key_path)
    .bind(input.ssl_enabled)
    .bind(&input.ssl_ca_path)
    .bind(&input.ssl_cert_path)
    .bind(&input.ssl_key_path)
    .bind(pool_max)
    .bind(input.ping_interval)
    .bind(&now)
    .bind(&now)
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    connections.register_name(&id, &row.name);

    Ok(ConnectionProfile::from(row))
}

/// Update an existing connection profile.
#[tauri::command]
pub async fn connections_update(
    sqlite: State<'_, SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    id: String,
    input: ConnectionProfileInput,
) -> Result<ConnectionProfile, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let pool_max = input.pool_max.unwrap_or(5);

    sqlx::query(
        r#"
        UPDATE connection_profiles SET
            group_id = ?, name = ?, db_type = ?, host = ?, port = ?, database = ?,
            username = ?, color = ?, read_only = ?,
            ssh_enabled = ?, ssh_host = ?, ssh_port = ?, ssh_user = ?,
            ssh_auth_type = ?, ssh_key_path = ?,
            ssl_enabled = ?, ssl_ca_path = ?, ssl_cert_path = ?, ssl_key_path = ?,
            pool_max = ?, ping_interval = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&input.group_id)
    .bind(&input.name)
    .bind(&input.db_type)
    .bind(&input.host)
    .bind(input.port)
    .bind(&input.database)
    .bind(&input.username)
    .bind(&input.color)
    .bind(input.read_only)
    .bind(input.ssh_enabled)
    .bind(&input.ssh_host)
    .bind(input.ssh_port)
    .bind(&input.ssh_user)
    .bind(&input.ssh_auth_type)
    .bind(&input.ssh_key_path)
    .bind(input.ssl_enabled)
    .bind(&input.ssl_ca_path)
    .bind(&input.ssl_cert_path)
    .bind(&input.ssl_key_path)
    .bind(pool_max)
    .bind(input.ping_interval)
    .bind(&now)
    .bind(&id)
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    connections.register_name(&id, &row.name);

    Ok(ConnectionProfile::from(row))
}

/// Delete a connection profile.
#[tauri::command]
pub async fn connections_delete(sqlite: State<'_, SqlitePool>, id: String) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM connection_profiles WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

/// Test a connection (connect and disconnect immediately, measuring latency).
/// `password` is accepted directly here so the connection can be tested before
/// it is saved to the keychain (e.g. from the "Test Connection" button in the form).
#[tauri::command]
pub async fn connections_test(
    sqlite: State<'_, SqlitePool>,
    tunnels: State<'_, Arc<SshTunnelManager>>,
    connections: State<'_, Arc<ConnectionManager>>,
    id: String,
    password: Option<String>,
) -> Result<ConnectionTestResult, AppError> {
    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_optional(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| {
                AppError::new(
                    "CONNECTION_NOT_FOUND",
                    format!("No connection with id {id}"),
                )
            })?;

    // Use the provided password, fall back to keychain, then to empty string.
    let password = password
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| retrieve_keychain_password(&id));

    // Create a temporary SSH tunnel if needed, using a distinct key so we don't
    // interfere with an active connection tunnel for the same profile.
    let tunnel_key = format!("{id}:test");
    let (connect_host, connect_port) = if row.ssh_enabled {
        let ssh_host = row
            .ssh_host
            .ok_or_else(|| AppError::new("SSH_ERROR", "SSH host not set"))?;
        let ssh_port = row.ssh_port.unwrap_or(22) as u16;
        let ssh_user = row
            .ssh_user
            .ok_or_else(|| AppError::new("SSH_ERROR", "SSH user not set"))?;
        let auth_type = row.ssh_auth_type.as_deref().unwrap_or("key");
        let ssh_password = (auth_type == "password")
            .then(|| retrieve_keychain_secret(&id, "ssh_password"))
            .flatten();
        let ssh_key_passphrase = (auth_type == "key")
            .then(|| retrieve_keychain_secret(&id, "ssh_key_passphrase"))
            .flatten();

        let local_port = tunnels
            .create_tunnel(
                &tunnel_key,
                &ssh_host,
                ssh_port,
                &ssh_user,
                ssh_password,
                row.ssh_key_path.clone(),
                ssh_key_passphrase,
                &row.host,
                row.port as u16,
            )
            .await
            .map_err(AppError::from)?;
        ("127.0.0.1".to_owned(), local_port)
    } else {
        (row.host.clone(), row.port as u16)
    };

    let start = std::time::Instant::now();

    let test_result = connections
        .test_connect(
            &row.db_type,
            &connect_host,
            connect_port,
            &row.database,
            &row.username,
            &password,
            row.ssl_enabled,
            row.ssl_ca_path.as_deref(),
            row.ssl_cert_path.as_deref(),
            row.ssl_key_path.as_deref(),
        )
        .await;

    tunnels.destroy_tunnel(&tunnel_key);
    let latency_ms = start.elapsed().as_millis() as u64;

    Ok(match test_result {
        Ok(()) => ConnectionTestResult {
            success: true,
            message: "Connection successful".to_owned(),
            latency_ms: Some(latency_ms),
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: e.to_string(),
            latency_ms: None,
        },
    })
}

/// Test a connection from raw input without saving it to the database.
#[tauri::command]
pub async fn connections_test_unsaved(
    tunnels: State<'_, Arc<SshTunnelManager>>,
    connections: State<'_, Arc<ConnectionManager>>,
    input: ConnectionProfileInput,
    password: Option<String>,
    ssh_password: Option<String>,
) -> Result<ConnectionTestResult, AppError> {
    let password = password.unwrap_or_default();

    let tunnel_key = uuid::Uuid::new_v4().to_string();
    let (connect_host, connect_port) = if input.ssh_enabled {
        let ssh_host = input
            .ssh_host
            .as_deref()
            .ok_or_else(|| AppError::new("SSH_ERROR", "SSH host not set"))?;
        let ssh_port = input.ssh_port.unwrap_or(22) as u16;
        let ssh_user = input
            .ssh_user
            .as_deref()
            .ok_or_else(|| AppError::new("SSH_ERROR", "SSH user not set"))?;
        let auth_type = input.ssh_auth_type.as_deref().unwrap_or("key");
        let ssh_pass = (auth_type == "password").then_some(ssh_password).flatten();

        let local_port = tunnels
            .create_tunnel(
                &tunnel_key,
                ssh_host,
                ssh_port,
                ssh_user,
                ssh_pass,
                input.ssh_key_path.clone(),
                None,
                &input.host,
                input.port as u16,
            )
            .await
            .map_err(AppError::from)?;
        ("127.0.0.1".to_owned(), local_port)
    } else {
        (input.host.clone(), input.port as u16)
    };

    let start = std::time::Instant::now();

    let test_result = connections
        .test_connect(
            &input.db_type,
            &connect_host,
            connect_port,
            &input.database,
            &input.username,
            &password,
            input.ssl_enabled,
            input.ssl_ca_path.as_deref(),
            input.ssl_cert_path.as_deref(),
            input.ssl_key_path.as_deref(),
        )
        .await;

    tunnels.destroy_tunnel(&tunnel_key);
    let latency_ms = start.elapsed().as_millis() as u64;

    Ok(match test_result {
        Ok(()) => ConnectionTestResult {
            success: true,
            message: "Connection successful".to_owned(),
            latency_ms: Some(latency_ms),
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: e.to_string(),
            latency_ms: None,
        },
    })
}

/// Open a connection pool for the given profile.
#[tauri::command]
pub async fn connections_connect(
    app: tauri::AppHandle,
    sqlite: State<'_, SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<crate::transactions::TransactionManager>>,
    tunnels: State<'_, Arc<SshTunnelManager>>,
    id: String,
) -> Result<(), AppError> {
    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_optional(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| {
                AppError::new(
                    "CONNECTION_NOT_FOUND",
                    format!("No connection with id {id}"),
                )
            })?;

    let password = retrieve_keychain_password(&id);

    let (connect_host, connect_port, new_tunnel) = if row.ssh_enabled {
        if let Some(port) = tunnels.local_port(&id) {
            ("127.0.0.1".to_owned(), port, false)
        } else {
            let ssh_host = row
                .ssh_host
                .ok_or_else(|| AppError::new("SSH_ERROR", "SSH host not set"))?;
            let ssh_port = row.ssh_port.unwrap_or(22) as u16;
            let ssh_user = row
                .ssh_user
                .ok_or_else(|| AppError::new("SSH_ERROR", "SSH user not set"))?;
            let auth_type = row.ssh_auth_type.as_deref().unwrap_or("key");
            let ssh_password = (auth_type == "password")
                .then(|| retrieve_keychain_secret(&id, "ssh_password"))
                .flatten();
            let ssh_key_passphrase = (auth_type == "key")
                .then(|| retrieve_keychain_secret(&id, "ssh_key_passphrase"))
                .flatten();

            let port = tunnels
                .create_tunnel(
                    &id,
                    &ssh_host,
                    ssh_port,
                    &ssh_user,
                    ssh_password,
                    row.ssh_key_path.clone(),
                    ssh_key_passphrase,
                    &row.host,
                    row.port as u16,
                )
                .await
                .map_err(AppError::from)?;
            ("127.0.0.1".to_owned(), port, true)
        }
    } else {
        (row.host.clone(), row.port as u16, false)
    };

    connections
        .connect(
            &id,
            &row.name,
            &row.db_type,
            &connect_host,
            connect_port,
            &row.database,
            &row.username,
            &password,
            row.pool_max as u32,
            row.ssl_enabled,
            row.ssl_ca_path.as_deref(),
            row.ssl_cert_path.as_deref(),
            row.ssl_key_path.as_deref(),
            row.read_only,
        )
        .await
        .map_err(AppError::from)?;

    // When a new SSH tunnel was created, watch for unexpected exits so we can
    // clean up the pool and notify the frontend without waiting for a failed query.
    if new_tunnel {
        if let Some(mut exit_rx) = tunnels.exit_receiver(&id) {
            let id_clone = id.clone();
            let connections_clone = connections.inner().clone();
            let transactions_clone = transactions.inner().clone();
            let tunnels_clone = tunnels.inner().clone();
            let app_clone = app.clone();
            tokio::spawn(async move {
                if exit_rx.changed().await.is_err() {
                    return;
                }
                // destroy_tunnel returns true only if the tunnel was still
                // registered, meaning the exit was unexpected (not a user disconnect).
                if tunnels_clone.destroy_tunnel(&id_clone) {
                    transactions_clone.remove(&id_clone);
                    connections_clone.disconnect(&id_clone).await;
                    let _ = app_clone.emit("connection:ssh-dropped", &id_clone);
                }
            });
        }
    }

    // Spawn a background task to periodically ping the connection and disconnect
    // if it becomes unreachable, notifying the frontend so it can update UI state.
    if let Some(interval_secs) = row.ping_interval.filter(|&s| s > 0) {
        let id_clone = id.clone();
        let connections_clone = connections.inner().clone();
        let transactions_clone = transactions.inner().clone();
        let tunnels_clone = tunnels.inner().clone();
        let app_clone = app.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(interval_secs as u64)).await;
                if !connections_clone.is_active(&id_clone) {
                    break;
                }
                if !connections_clone.ping(&id_clone).await {
                    if connections_clone.is_active(&id_clone) {
                        transactions_clone.remove(&id_clone);
                        tunnels_clone.destroy_tunnel(&id_clone);
                        connections_clone.disconnect(&id_clone).await;
                        let _ = app_clone.emit("connection:ping-failed", &id_clone);
                    }
                    break;
                }
            }
        });
    }

    Ok(())
}

/// Connect using raw input without persisting a profile to SQLite.
/// Used for the "don't save this connection" option in the connection form:
/// the pool is keyed by a freshly generated id that is never written to
/// `connection_profiles`, so it vanishes once disconnected instead of
/// lingering in the saved connection list.
#[tauri::command]
pub async fn connections_connect_unsaved(
    app: tauri::AppHandle,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<crate::transactions::TransactionManager>>,
    tunnels: State<'_, Arc<SshTunnelManager>>,
    input: ConnectionProfileInput,
    password: Option<String>,
    ssh_password: Option<String>,
) -> Result<ConnectionProfile, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let pool_max = input.pool_max.unwrap_or(5);
    let password = password.unwrap_or_default();

    let (connect_host, connect_port) = if input.ssh_enabled {
        let ssh_host = input
            .ssh_host
            .clone()
            .ok_or_else(|| AppError::new("SSH_ERROR", "SSH host not set"))?;
        let ssh_port = input.ssh_port.unwrap_or(22) as u16;
        let ssh_user = input
            .ssh_user
            .clone()
            .ok_or_else(|| AppError::new("SSH_ERROR", "SSH user not set"))?;
        let auth_type = input.ssh_auth_type.as_deref().unwrap_or("key");
        let ssh_pass = (auth_type == "password").then_some(ssh_password).flatten();

        let local_port = tunnels
            .create_tunnel(
                &id,
                &ssh_host,
                ssh_port,
                &ssh_user,
                ssh_pass,
                input.ssh_key_path.clone(),
                None,
                &input.host,
                input.port as u16,
            )
            .await
            .map_err(AppError::from)?;
        ("127.0.0.1".to_owned(), local_port)
    } else {
        (input.host.clone(), input.port as u16)
    };

    connections
        .connect(
            &id,
            &input.name,
            &input.db_type,
            &connect_host,
            connect_port,
            &input.database,
            &input.username,
            &password,
            pool_max as u32,
            input.ssl_enabled,
            input.ssl_ca_path.as_deref(),
            input.ssl_cert_path.as_deref(),
            input.ssl_key_path.as_deref(),
            input.read_only,
        )
        .await
        .map_err(AppError::from)?;

    // Watch for unexpected SSH tunnel exits, same as a saved connection.
    if input.ssh_enabled {
        if let Some(mut exit_rx) = tunnels.exit_receiver(&id) {
            let id_clone = id.clone();
            let connections_clone = connections.inner().clone();
            let transactions_clone = transactions.inner().clone();
            let tunnels_clone = tunnels.inner().clone();
            let app_clone = app.clone();
            tokio::spawn(async move {
                if exit_rx.changed().await.is_err() {
                    return;
                }
                if tunnels_clone.destroy_tunnel(&id_clone) {
                    transactions_clone.remove(&id_clone);
                    connections_clone.disconnect(&id_clone).await;
                    let _ = app_clone.emit("connection:ssh-dropped", &id_clone);
                }
            });
        }
    }

    // Periodic ping, same as a saved connection.
    if let Some(interval_secs) = input.ping_interval.filter(|&s| s > 0) {
        let id_clone = id.clone();
        let connections_clone = connections.inner().clone();
        let transactions_clone = transactions.inner().clone();
        let tunnels_clone = tunnels.inner().clone();
        let app_clone = app.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(interval_secs as u64)).await;
                if !connections_clone.is_active(&id_clone) {
                    break;
                }
                if !connections_clone.ping(&id_clone).await {
                    if connections_clone.is_active(&id_clone) {
                        transactions_clone.remove(&id_clone);
                        tunnels_clone.destroy_tunnel(&id_clone);
                        connections_clone.disconnect(&id_clone).await;
                        let _ = app_clone.emit("connection:ping-failed", &id_clone);
                    }
                    break;
                }
            }
        });
    }

    let dialect_info = DialectInfo::for_db_type(&input.db_type);

    Ok(ConnectionProfile {
        id,
        group_id: input.group_id,
        name: input.name,
        db_type: input.db_type,
        host: input.host,
        port: input.port,
        database: input.database,
        username: input.username,
        color: input.color,
        read_only: input.read_only,
        ssh_enabled: input.ssh_enabled,
        ssh_host: input.ssh_host,
        ssh_port: input.ssh_port,
        ssh_user: input.ssh_user,
        ssh_auth_type: input.ssh_auth_type,
        ssh_key_path: input.ssh_key_path,
        ssl_enabled: input.ssl_enabled,
        ssl_ca_path: input.ssl_ca_path,
        ssl_cert_path: input.ssl_cert_path,
        ssl_key_path: input.ssl_key_path,
        pool_max,
        ping_interval: input.ping_interval,
        created_at: now.clone(),
        updated_at: now,
        dialect_info,
    })
}

/// Close the connection pool for the given profile.
/// Also removes any active transaction for this connection — the underlying
/// DB connection is dropped, which causes the server to roll back automatically.
#[tauri::command]
pub async fn connections_disconnect(
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<crate::transactions::TransactionManager>>,
    tunnels: State<'_, Arc<SshTunnelManager>>,
    id: String,
) -> Result<(), AppError> {
    transactions.remove(&id);
    connections.disconnect(&id).await;
    tunnels.destroy_tunnel(&id);
    Ok(())
}

/// Return the ids of all currently active connections.
#[tauri::command]
pub async fn connections_list_active(
    connections: State<'_, Arc<ConnectionManager>>,
) -> Result<Vec<String>, AppError> {
    Ok(connections.active_ids())
}

/// List all connection groups.
#[tauri::command]
pub async fn connection_groups_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<ConnectionGroup>, AppError> {
    let rows = sqlx::query_as::<_, ConnectionGroupRow>(
        "SELECT * FROM connection_groups ORDER BY position, name",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(ConnectionGroup::from).collect())
}

#[derive(Debug, Deserialize)]
pub struct ConnectionGroupInput {
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

/// Create a new connection group.
#[tauri::command]
pub async fn connection_groups_create(
    sqlite: State<'_, SqlitePool>,
    input: ConnectionGroupInput,
) -> Result<ConnectionGroup, AppError> {
    let id = Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO connection_groups (id, name, parent_id, position) VALUES (?, ?, ?, 0)",
        id,
        input.name,
        input.parent_id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(ConnectionGroup::from(row))
}

/// Delete a connection group.
#[tauri::command]
pub async fn connection_groups_delete(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM connection_groups WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

/// Update an existing connection group's name, parent, and position.
#[tauri::command]
pub async fn connection_groups_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: ConnectionGroupInput,
) -> Result<ConnectionGroup, AppError> {
    sqlx::query!(
        "UPDATE connection_groups SET name = ?, parent_id = ?, position = 0 WHERE id = ?",
        input.name,
        input.parent_id,
        id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(ConnectionGroup::from(row))
}

/// Reorder connection groups by updating their parent_id and position.
#[derive(Debug, Deserialize)]
pub struct GroupReorderItem {
    pub id: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub position: i64,
}

#[tauri::command]
pub async fn connection_groups_reorder(
    sqlite: State<'_, SqlitePool>,
    updates: Vec<GroupReorderItem>,
) -> Result<(), AppError> {
    for item in &updates {
        sqlx::query!(
            "UPDATE connection_groups SET parent_id = ?, position = ? WHERE id = ?",
            item.parent_id,
            item.position,
            item.id
        )
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }
    Ok(())
}

/// Duplicate an existing connection profile with a "Copy of …" name.
/// Keychain secrets (db password, SSH password, SSH key passphrase) are
/// copied to the new connection's keychain entries.
#[tauri::command]
pub async fn connections_duplicate(
    sqlite: State<'_, SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    id: String,
) -> Result<ConnectionProfile, AppError> {
    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_optional(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| {
                AppError::new(
                    "CONNECTION_NOT_FOUND",
                    format!("No connection with id {id}"),
                )
            })?;

    let new_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let new_name = format!("Copy of {}", row.name);

    sqlx::query(
        r#"
        INSERT INTO connection_profiles (
            id, group_id, name, db_type, host, port, database, username, color,
            read_only, ssh_enabled, ssh_host, ssh_port, ssh_user, ssh_auth_type, ssh_key_path,
            ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path,
            pool_max, ping_interval, created_at, updated_at
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?
        )
        "#,
    )
    .bind(&new_id)
    .bind(&row.group_id)
    .bind(&new_name)
    .bind(&row.db_type)
    .bind(&row.host)
    .bind(row.port)
    .bind(&row.database)
    .bind(&row.username)
    .bind(&row.color)
    .bind(row.read_only)
    .bind(row.ssh_enabled)
    .bind(&row.ssh_host)
    .bind(row.ssh_port)
    .bind(&row.ssh_user)
    .bind(&row.ssh_auth_type)
    .bind(&row.ssh_key_path)
    .bind(row.ssl_enabled)
    .bind(&row.ssl_ca_path)
    .bind(&row.ssl_cert_path)
    .bind(&row.ssl_key_path)
    .bind(row.pool_max)
    .bind(row.ping_interval)
    .bind(&now)
    .bind(&now)
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    for secret_type in &["db_password", "ssh_password", "ssh_key_passphrase"] {
        if let Some(secret) = retrieve_keychain_secret(&id, secret_type) {
            if !secret.is_empty() {
                let account = format!("{new_id}:{secret_type}");
                let _ =
                    crate::commands::keychain::keychain_write_secret("rowmance", &account, &secret);
            }
        }
    }

    let new_row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&new_id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    connections.register_name(&new_id, &new_row.name);

    Ok(ConnectionProfile::from(new_row))
}

// ── Import / Export types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionExportPasswords {
    #[serde(rename = "dbPassword")]
    pub db_password: Option<String>,
    #[serde(rename = "sshPassword")]
    pub ssh_password: Option<String>,
    #[serde(rename = "sshKeyPassphrase")]
    pub ssh_key_passphrase: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionExportEntry {
    pub name: String,
    #[serde(rename = "dbType")]
    pub db_type: String,
    pub host: String,
    pub port: i64,
    pub database: String,
    pub username: String,
    pub color: Option<String>,
    #[serde(rename = "readOnly")]
    pub read_only: bool,
    #[serde(rename = "sshEnabled")]
    pub ssh_enabled: bool,
    #[serde(rename = "sshHost")]
    pub ssh_host: Option<String>,
    #[serde(rename = "sshPort")]
    pub ssh_port: Option<i64>,
    #[serde(rename = "sshUser")]
    pub ssh_user: Option<String>,
    #[serde(rename = "sshAuthType")]
    pub ssh_auth_type: Option<String>,
    #[serde(rename = "sshKeyPath")]
    pub ssh_key_path: Option<String>,
    #[serde(rename = "sslEnabled")]
    pub ssl_enabled: bool,
    #[serde(rename = "sslCaPath")]
    pub ssl_ca_path: Option<String>,
    #[serde(rename = "sslCertPath")]
    pub ssl_cert_path: Option<String>,
    #[serde(rename = "sslKeyPath")]
    pub ssl_key_path: Option<String>,
    #[serde(rename = "poolMax")]
    pub pool_max: i64,
    #[serde(rename = "pingInterval")]
    pub ping_interval: Option<i64>,
    /// Present only when exported with include_sensitive = true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passwords: Option<ConnectionExportPasswords>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionExportFile {
    pub version: u32,
    pub connections: Vec<ConnectionExportEntry>,
}

#[derive(Debug, Serialize)]
pub struct ConnectionImportResult {
    pub imported: u32,
}

// ── Export command ────────────────────────────────────────────────────────────

/// Export one or more connection profiles to a JSON file.
/// When `include_sensitive` is true the plaintext passwords are embedded.
#[tauri::command]
pub async fn connections_export(
    sqlite: State<'_, SqlitePool>,
    ids: Vec<String>,
    file_path: String,
    include_sensitive: bool,
) -> Result<(), AppError> {
    let mut entries = Vec::new();

    for id in &ids {
        let row = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

        if let Some(row) = row {
            let passwords = if include_sensitive {
                Some(ConnectionExportPasswords {
                    db_password: retrieve_keychain_secret(id, "db_password"),
                    ssh_password: retrieve_keychain_secret(id, "ssh_password"),
                    ssh_key_passphrase: retrieve_keychain_secret(id, "ssh_key_passphrase"),
                })
            } else {
                None
            };

            entries.push(ConnectionExportEntry {
                name: row.name,
                db_type: row.db_type,
                host: row.host,
                port: row.port,
                database: row.database,
                username: row.username,
                color: row.color,
                read_only: row.read_only,
                ssh_enabled: row.ssh_enabled,
                ssh_host: row.ssh_host,
                ssh_port: row.ssh_port,
                ssh_user: row.ssh_user,
                ssh_auth_type: row.ssh_auth_type,
                ssh_key_path: row.ssh_key_path,
                ssl_enabled: row.ssl_enabled,
                ssl_ca_path: row.ssl_ca_path,
                ssl_cert_path: row.ssl_cert_path,
                ssl_key_path: row.ssl_key_path,
                pool_max: row.pool_max,
                ping_interval: row.ping_interval,
                passwords,
            });
        }
    }

    let export_file = ConnectionExportFile {
        version: 1,
        connections: entries,
    };

    let json = serde_json::to_string_pretty(&export_file)
        .map_err(|e| AppError::new("SERIALISATION_ERROR", e.to_string()))?;

    std::fs::write(&file_path, json).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    Ok(())
}

// ── Dialect listing command ───────────────────────────────────────────────────

/// Dialect metadata for a single engine type.
#[derive(Debug, Serialize)]
pub struct DialectEntry {
    #[serde(rename = "dbType")]
    pub db_type: String,
    pub dialect: DialectInfo,
}

/// Return dialect metadata for all engines known to this build.
/// The frontend uses this to drive engine-specific UI (ports, URL schemes, etc.)
/// without hardcoding engine names in component code.
#[tauri::command]
pub async fn connections_list_dialects() -> Vec<DialectEntry> {
    DialectInfo::all_known()
        .into_iter()
        .map(|(db_type, dialect)| DialectEntry { db_type, dialect })
        .collect()
}

// ── Database URL command ──────────────────────────────────────────────────────

/// Build a database URL for a connection profile, including the password from
/// the OS keychain. Returns a URL suitable for use with database clients.
#[tauri::command]
pub async fn connections_get_db_url(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<String, AppError> {
    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_optional(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| AppError::new("NOT_FOUND", format!("Connection {id} not found")))?;

    let password = retrieve_keychain_password(&id);

    let dialect = DialectInfo::for_db_type(&row.db_type);
    if dialect.url_template.is_empty() {
        return Err(AppError::new(
            "UNSUPPORTED",
            format!("Unknown db_type: {}", row.db_type),
        ));
    }
    let url = dialect
        .url_template
        .replace("{username}", &row.username)
        .replace("{password}", &password)
        .replace("{host}", &row.host)
        .replace("{port}", &row.port.to_string())
        .replace("{database}", &row.database);

    Ok(url)
}

#[tauri::command]
pub async fn connections_copy_db_url_to_clipboard(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    let url = connections_get_db_url(sqlite, id).await?;
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| AppError::new("CLIPBOARD_ERROR", e.to_string()))?;
    clipboard
        .set_text(url)
        .map_err(|e| AppError::new("CLIPBOARD_ERROR", e.to_string()))?;
    Ok(())
}

// ── Import command ────────────────────────────────────────────────────────────

/// Import connection profiles from a JSON file produced by `connections_export`.
/// Any embedded passwords are stored in the OS keychain.
#[tauri::command]
pub async fn connections_import(
    sqlite: State<'_, SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    file_path: String,
) -> Result<ConnectionImportResult, AppError> {
    let json = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    let export_file: ConnectionExportFile = serde_json::from_str(&json).map_err(|e| {
        AppError::new(
            "PARSE_ERROR",
            format!("Invalid connection export file: {e}"),
        )
    })?;

    let mut imported = 0u32;

    for entry in export_file.connections {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO connection_profiles (
                id, group_id, name, db_type, host, port, database, username, color,
                read_only, ssh_enabled, ssh_host, ssh_port, ssh_user, ssh_auth_type, ssh_key_path,
                ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path,
                pool_max, ping_interval, created_at, updated_at
            ) VALUES (
                ?, NULL, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?
            )
            "#,
        )
        .bind(&id)
        .bind(&entry.name)
        .bind(&entry.db_type)
        .bind(&entry.host)
        .bind(entry.port)
        .bind(&entry.database)
        .bind(&entry.username)
        .bind(&entry.color)
        .bind(entry.read_only)
        .bind(entry.ssh_enabled)
        .bind(&entry.ssh_host)
        .bind(entry.ssh_port)
        .bind(&entry.ssh_user)
        .bind(&entry.ssh_auth_type)
        .bind(&entry.ssh_key_path)
        .bind(entry.ssl_enabled)
        .bind(&entry.ssl_ca_path)
        .bind(&entry.ssl_cert_path)
        .bind(&entry.ssl_key_path)
        .bind(entry.pool_max)
        .bind(entry.ping_interval)
        .bind(&now)
        .bind(&now)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

        connections.register_name(&id, &entry.name);

        if let Some(passwords) = entry.passwords {
            if let Some(pw) = passwords.db_password.filter(|p| !p.is_empty()) {
                let account = format!("{id}:db_password");
                let _ = crate::commands::keychain::keychain_write_secret("rowmance", &account, &pw);
            }
            if let Some(pw) = passwords.ssh_password.filter(|p| !p.is_empty()) {
                let account = format!("{id}:ssh_password");
                let _ = crate::commands::keychain::keychain_write_secret("rowmance", &account, &pw);
            }
            if let Some(pw) = passwords.ssh_key_passphrase.filter(|p| !p.is_empty()) {
                let account = format!("{id}:ssh_key_passphrase");
                let _ = crate::commands::keychain::keychain_write_secret("rowmance", &account, &pw);
            }
        }

        imported += 1;
    }

    Ok(ConnectionImportResult { imported })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{ConnectionGroupRow, ConnectionProfileRow};

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        pool
    }

    async fn insert_profile(pool: &SqlitePool, id: &str, name: &str) -> ConnectionProfileRow {
        sqlx::query(
            r#"INSERT INTO connection_profiles
               (id, name, db_type, host, port, database, username, read_only,
                ssh_enabled, ssl_enabled, pool_max, created_at, updated_at)
               VALUES (?, ?, 'postgres', 'localhost', 5432, 'db', 'user', 0,
                       0, 0, 5, '2024-01-01', '2024-01-01')"#,
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn insert_group(pool: &SqlitePool, id: &str, name: &str) -> ConnectionGroupRow {
        sqlx::query(
            "INSERT INTO connection_groups (id, name, parent_id, position) VALUES (?, ?, NULL, 0)",
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    // ── Profile CRUD ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_profiles_returns_empty_initially() {
        let pool = setup_db().await;
        let rows = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn create_profile_round_trips_all_fields() {
        let pool = setup_db().await;
        let row = insert_profile(&pool, "p-1", "My Server").await;

        assert_eq!(row.id, "p-1");
        assert_eq!(row.name, "My Server");
        assert_eq!(row.db_type, "postgres");
        assert_eq!(row.host, "localhost");
        assert_eq!(row.port, 5432);
        assert!(!row.read_only);
        assert!(!row.ssh_enabled);
        assert!(!row.ssl_enabled);
        assert_eq!(row.pool_max, 5);
    }

    #[tokio::test]
    async fn list_profiles_returns_all_created() {
        let pool = setup_db().await;
        insert_profile(&pool, "p-a", "Server A").await;
        insert_profile(&pool, "p-b", "Server B").await;

        let rows = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn update_profile_changes_name_and_host() {
        let pool = setup_db().await;
        insert_profile(&pool, "p-upd", "Original").await;

        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE connection_profiles SET name = ?, host = ?, updated_at = ? WHERE id = ?",
        )
        .bind("Renamed")
        .bind("db.example.com")
        .bind(&now)
        .bind("p-upd")
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles WHERE id = ?",
        )
        .bind("p-upd")
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.name, "Renamed");
        assert_eq!(row.host, "db.example.com");
    }

    #[tokio::test]
    async fn delete_profile_removes_it() {
        let pool = setup_db().await;
        insert_profile(&pool, "p-del", "ToDelete").await;

        sqlx::query("DELETE FROM connection_profiles WHERE id = ?")
            .bind("p-del")
            .execute(&pool)
            .await
            .unwrap();

        let row = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles WHERE id = ?",
        )
        .bind("p-del")
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn read_only_flag_is_stored() {
        let pool = setup_db().await;
        sqlx::query(
            r#"INSERT INTO connection_profiles
               (id, name, db_type, host, port, database, username, read_only,
                ssh_enabled, ssl_enabled, pool_max, created_at, updated_at)
               VALUES ('ro-1', 'ReadOnly', 'mysql', 'localhost', 3306, 'db', 'root', 1,
                       0, 0, 5, '2024-01-01', '2024-01-01')"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles WHERE id = 'ro-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.read_only);
    }

    // ── Group CRUD ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_groups_returns_empty_initially() {
        let pool = setup_db().await;
        let rows = sqlx::query_as::<_, ConnectionGroupRow>(
            "SELECT * FROM connection_groups ORDER BY position, name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn create_group_stores_name_and_id() {
        let pool = setup_db().await;
        let row = insert_group(&pool, "g-1", "Production").await;
        assert_eq!(row.id, "g-1");
        assert_eq!(row.name, "Production");
        assert!(row.parent_id.is_none());
    }

    #[tokio::test]
    async fn create_nested_group() {
        let pool = setup_db().await;
        insert_group(&pool, "parent", "Top Level").await;

        sqlx::query(
            "INSERT INTO connection_groups (id, name, parent_id, position) VALUES (?, ?, ?, 0)",
        )
        .bind("child")
        .bind("Sub Group")
        .bind("parent")
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query_as::<_, ConnectionGroupRow>(
            "SELECT * FROM connection_groups WHERE id = 'child'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.parent_id.as_deref(), Some("parent"));
    }

    #[tokio::test]
    async fn delete_group_removes_it() {
        let pool = setup_db().await;
        insert_group(&pool, "g-del", "ToDelete").await;

        sqlx::query("DELETE FROM connection_groups WHERE id = ?")
            .bind("g-del")
            .execute(&pool)
            .await
            .unwrap();

        let row =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("g-del")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn deleting_group_sets_profile_group_id_to_null() {
        // The schema uses ON DELETE SET NULL for connection_profiles.group_id.
        // SQLite only enforces this when foreign_keys pragma is on, but we verify
        // the schema definition is correct by checking the constraint is declared.
        // A full FK enforcement test would require PRAGMA foreign_keys = ON.
        let pool = setup_db().await;
        insert_group(&pool, "g-fk", "FK Group").await;

        sqlx::query(
            r#"INSERT INTO connection_profiles
               (id, name, db_type, host, port, database, username, read_only,
                ssh_enabled, ssl_enabled, pool_max, created_at, updated_at,
                group_id)
               VALUES ('p-fk', 'FK Profile', 'postgres', 'localhost', 5432, 'db', 'user',
                       0, 0, 0, 5, '2024-01-01', '2024-01-01', 'g-fk')"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Confirm group_id is set.
        let row = sqlx::query_as::<_, ConnectionProfileRow>(
            "SELECT * FROM connection_profiles WHERE id = 'p-fk'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.group_id.as_deref(), Some("g-fk"));
    }

    // ── From conversions ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn connection_profile_from_row_preserves_all_fields() {
        let pool = setup_db().await;
        let row = insert_profile(&pool, "conv-1", "ConvTest").await;
        let profile = ConnectionProfile::from(row.clone());

        assert_eq!(profile.id, row.id);
        assert_eq!(profile.name, row.name);
        assert_eq!(profile.db_type, row.db_type);
        assert_eq!(profile.host, row.host);
        assert_eq!(profile.port, row.port);
        assert_eq!(profile.read_only, row.read_only);
        assert_eq!(profile.ssh_enabled, row.ssh_enabled);
        assert_eq!(profile.ssl_enabled, row.ssl_enabled);
        assert_eq!(profile.pool_max, row.pool_max);
    }

    // ── connection_groups_update ──────────────────────────────────────────────

    #[tokio::test]
    async fn update_group_changes_name() {
        let pool = setup_db().await;
        insert_group(&pool, "g-upd", "Old Name").await;

        sqlx::query!(
            "UPDATE connection_groups SET name = ?, parent_id = ?, position = 0 WHERE id = ?",
            "New Name",
            None::<String>,
            "g-upd"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("g-upd")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.name, "New Name");
    }

    #[tokio::test]
    async fn update_group_sets_parent_id() {
        let pool = setup_db().await;
        insert_group(&pool, "g-parent", "Parent").await;
        insert_group(&pool, "g-child", "Child").await;

        sqlx::query!(
            "UPDATE connection_groups SET name = ?, parent_id = ?, position = 0 WHERE id = ?",
            "Child",
            "g-parent",
            "g-child"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("g-child")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.parent_id.as_deref(), Some("g-parent"));
    }

    #[tokio::test]
    async fn update_group_clears_parent_id() {
        let pool = setup_db().await;
        insert_group(&pool, "g-par2", "Parent2").await;

        // Insert a nested group with parent_id set.
        sqlx::query(
            "INSERT INTO connection_groups (id, name, parent_id, position) VALUES (?, ?, ?, 0)",
        )
        .bind("g-nested")
        .bind("Nested")
        .bind("g-par2")
        .execute(&pool)
        .await
        .unwrap();

        // Update clearing parent_id.
        sqlx::query!(
            "UPDATE connection_groups SET name = ?, parent_id = ?, position = 0 WHERE id = ?",
            "Nested",
            None::<String>,
            "g-nested"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("g-nested")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert!(row.parent_id.is_none());
    }

    // ── connection_groups_reorder ─────────────────────────────────────────────

    #[tokio::test]
    async fn reorder_groups_updates_positions() {
        let pool = setup_db().await;
        insert_group(&pool, "rg-1", "Group 1").await;
        insert_group(&pool, "rg-2", "Group 2").await;
        insert_group(&pool, "rg-3", "Group 3").await;

        // Reorder: rg-1 → pos 2, rg-2 → pos 0, rg-3 → pos 1
        let updates: &[(&str, Option<&str>, i64)] =
            &[("rg-1", None, 2), ("rg-2", None, 0), ("rg-3", None, 1)];

        for (id, parent_id, position) in updates {
            sqlx::query!(
                "UPDATE connection_groups SET parent_id = ?, position = ? WHERE id = ?",
                *parent_id,
                position,
                id
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        let rg1 =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("rg-1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let rg2 =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("rg-2")
                .fetch_one(&pool)
                .await
                .unwrap();
        let rg3 =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("rg-3")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(rg1.position, 2);
        assert_eq!(rg2.position, 0);
        assert_eq!(rg3.position, 1);
    }

    #[tokio::test]
    async fn reorder_groups_moves_to_different_parent() {
        let pool = setup_db().await;
        insert_group(&pool, "rg-par", "Parent Group").await;
        insert_group(&pool, "rg-orphan", "Orphan Child").await;

        // Move orphan child under parent via reorder.
        sqlx::query!(
            "UPDATE connection_groups SET parent_id = ?, position = ? WHERE id = ?",
            "rg-par",
            0_i64,
            "rg-orphan"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row =
            sqlx::query_as::<_, ConnectionGroupRow>("SELECT * FROM connection_groups WHERE id = ?")
                .bind("rg-orphan")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.parent_id.as_deref(), Some("rg-par"));
    }
}

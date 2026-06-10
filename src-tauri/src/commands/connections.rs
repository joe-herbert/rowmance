/// Tauri commands for managing connection profiles and groups.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::connections::pool_manager::ConnectionManager;
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
    #[serde(rename = "poolMin")]
    pub pool_min: i64,
    #[serde(rename = "poolMax")]
    pub pool_max: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

impl From<ConnectionProfileRow> for ConnectionProfile {
    fn from(r: ConnectionProfileRow) -> Self {
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
            pool_min: r.pool_min,
            pool_max: r.pool_max,
            created_at: r.created_at,
            updated_at: r.updated_at,
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
    pub password: Option<String>,
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
    #[serde(rename = "poolMin")]
    pub pool_min: Option<i64>,
    #[serde(rename = "poolMax")]
    pub pool_max: Option<i64>,
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
    input: ConnectionProfileInput,
) -> Result<ConnectionProfile, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let pool_min = input.pool_min.unwrap_or(1);
    let pool_max = input.pool_max.unwrap_or(5);

    sqlx::query(
        r#"
        INSERT INTO connection_profiles (
            id, group_id, name, db_type, host, port, database, username, password, color,
            read_only, ssh_enabled, ssh_host, ssh_port, ssh_user, ssh_auth_type, ssh_key_path,
            ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path,
            pool_min, pool_max, created_at, updated_at
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
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
    .bind(&input.password)
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
    .bind(pool_min)
    .bind(pool_max)
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

    Ok(ConnectionProfile::from(row))
}

/// Update an existing connection profile.
#[tauri::command]
pub async fn connections_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: ConnectionProfileInput,
) -> Result<ConnectionProfile, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let pool_min = input.pool_min.unwrap_or(1);
    let pool_max = input.pool_max.unwrap_or(5);

    // Only overwrite the stored password if the user provided a new one.
    if let Some(ref pw) = input.password {
        sqlx::query("UPDATE connection_profiles SET password = ? WHERE id = ?")
            .bind(pw)
            .bind(&id)
            .execute(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }

    sqlx::query!(
        r#"
        UPDATE connection_profiles SET
            group_id = ?, name = ?, db_type = ?, host = ?, port = ?, database = ?,
            username = ?, color = ?, read_only = ?,
            ssh_enabled = ?, ssh_host = ?, ssh_port = ?, ssh_user = ?,
            ssh_auth_type = ?, ssh_key_path = ?,
            ssl_enabled = ?, ssl_ca_path = ?, ssl_cert_path = ?, ssl_key_path = ?,
            pool_min = ?, pool_max = ?, updated_at = ?
        WHERE id = ?
        "#,
        input.group_id,
        input.name,
        input.db_type,
        input.host,
        input.port,
        input.database,
        input.username,
        input.color,
        input.read_only,
        input.ssh_enabled,
        input.ssh_host,
        input.ssh_port,
        input.ssh_user,
        input.ssh_auth_type,
        input.ssh_key_path,
        input.ssl_enabled,
        input.ssl_ca_path,
        input.ssl_cert_path,
        input.ssl_key_path,
        pool_min,
        pool_max,
        now,
        id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

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
#[tauri::command]
pub async fn connections_test(
    sqlite: State<'_, SqlitePool>,
    id: String,
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

    let password = row.password.clone().unwrap_or_default();
    let start = std::time::Instant::now();

    let result = match row.db_type.as_str() {
        "mysql" | "mariadb" => {
            let url = format!(
                "mysql://{}:{}@{}:{}/{}",
                row.username, password, row.host, row.port, row.database
            );
            sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .map(|_| ())
        }
        "postgres" => {
            let url = format!(
                "postgres://{}:{}@{}:{}/{}",
                row.username, password, row.host, row.port, row.database
            );
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .map(|_| ())
        }
        _ => {
            return Ok(ConnectionTestResult {
                success: false,
                message: format!("Unknown db_type: {}", row.db_type),
                latency_ms: None,
            });
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(_) => Ok(ConnectionTestResult {
            success: true,
            message: "Connection successful".to_owned(),
            latency_ms: Some(latency_ms),
        }),
        Err(e) => Ok(ConnectionTestResult {
            success: false,
            message: e.to_string(),
            latency_ms: None,
        }),
    }
}

/// Open a connection pool for the given profile.
#[tauri::command]
pub async fn connections_connect(
    sqlite: State<'_, SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
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

    let password = row.password.clone().unwrap_or_default();

    connections
        .connect(
            &id,
            &row.db_type,
            &row.host,
            row.port as u16,
            &row.database,
            &row.username,
            &password,
            row.pool_min as u32,
            row.pool_max as u32,
        )
        .await
        .map_err(AppError::from)?;

    Ok(())
}

/// Close the connection pool for the given profile.
#[tauri::command]
pub async fn connections_disconnect(
    connections: State<'_, Arc<ConnectionManager>>,
    id: String,
) -> Result<(), AppError> {
    connections.disconnect(&id).await;
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

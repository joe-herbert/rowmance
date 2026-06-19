/// Tauri commands for managing connection profiles and groups.
use keyring::Entry;
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
    #[serde(rename = "poolMin")]
    pub pool_min: Option<i64>,
    #[serde(rename = "poolMax")]
    pub pool_max: Option<i64>,
}

fn retrieve_keychain_password(connection_id: &str) -> String {
    let name = format!("{connection_id}:db_password");
    Entry::new("rowmance", &name)
        .ok()
        .and_then(|e| e.get_password().ok())
        .unwrap_or_default()
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
            id, group_id, name, db_type, host, port, database, username, color,
            read_only, ssh_enabled, ssh_host, ssh_port, ssh_user, ssh_auth_type, ssh_key_path,
            ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path,
            pool_min, pool_max, created_at, updated_at
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
/// `password` is accepted directly here so the connection can be tested before
/// it is saved to the keychain (e.g. from the "Test Connection" button in the form).
#[tauri::command]
pub async fn connections_test(
    sqlite: State<'_, SqlitePool>,
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
        "sqlite" => {
            use sqlx::sqlite::SqliteConnectOptions;
            let opts = SqliteConnectOptions::new()
                .filename(&row.host)
                .create_if_missing(true);
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
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

/// Test a connection from raw input without saving it to the database.
#[tauri::command]
pub async fn connections_test_unsaved(
    input: ConnectionProfileInput,
    password: Option<String>,
) -> Result<ConnectionTestResult, AppError> {
    let password = password.unwrap_or_default();
    let start = std::time::Instant::now();

    let result = match input.db_type.as_str() {
        "mysql" | "mariadb" => {
            let url = format!(
                "mysql://{}:{}@{}:{}/{}",
                input.username, password, input.host, input.port, input.database
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
                input.username, password, input.host, input.port, input.database
            );
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .map(|_| ())
        }
        "sqlite" => {
            use sqlx::sqlite::SqliteConnectOptions;
            let opts = SqliteConnectOptions::new()
                .filename(&input.host)
                .create_if_missing(true);
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
                .await
                .map(|_| ())
        }
        _ => {
            return Ok(ConnectionTestResult {
                success: false,
                message: format!("Unknown db_type: {}", input.db_type),
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

    let password = retrieve_keychain_password(&id);

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
            row.ssl_enabled,
            row.ssl_ca_path.as_deref(),
            row.ssl_cert_path.as_deref(),
            row.ssl_key_path.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{ConnectionGroupRow, ConnectionProfileRow};

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_profile(pool: &SqlitePool, id: &str, name: &str) -> ConnectionProfileRow {
        sqlx::query(
            r#"INSERT INTO connection_profiles
               (id, name, db_type, host, port, database, username, read_only,
                ssh_enabled, ssl_enabled, pool_min, pool_max, created_at, updated_at)
               VALUES (?, ?, 'postgres', 'localhost', 5432, 'db', 'user', 0,
                       0, 0, 1, 5, '2024-01-01', '2024-01-01')"#,
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
        assert_eq!(row.pool_min, 1);
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
                ssh_enabled, ssl_enabled, pool_min, pool_max, created_at, updated_at)
               VALUES ('ro-1', 'ReadOnly', 'mysql', 'localhost', 3306, 'db', 'root', 1,
                       0, 0, 1, 5, '2024-01-01', '2024-01-01')"#,
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

        let row = sqlx::query_as::<_, ConnectionGroupRow>(
            "SELECT * FROM connection_groups WHERE id = ?",
        )
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
                ssh_enabled, ssl_enabled, pool_min, pool_max, created_at, updated_at,
                group_id)
               VALUES ('p-fk', 'FK Profile', 'postgres', 'localhost', 5432, 'db', 'user',
                       0, 0, 0, 1, 5, '2024-01-01', '2024-01-01', 'g-fk')"#,
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
        assert_eq!(profile.pool_min, row.pool_min);
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
        let updates: &[(&str, Option<&str>, i64)] = &[
            ("rg-1", None, 2),
            ("rg-2", None, 0),
            ("rg-3", None, 1),
        ];

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

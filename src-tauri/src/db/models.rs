/// SQLite row structs for the local config database.
/// These types map 1-to-1 to the tables in 0001_init.sql and are
/// separate from the IPC types returned to the frontend.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConnectionProfileRow {
    pub id: String,
    pub group_id: Option<String>,
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: i64,
    pub database: String,
    pub username: String,
    pub password: Option<String>,
    pub color: Option<String>,
    pub read_only: bool,
    pub ssh_enabled: bool,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<i64>,
    pub ssh_user: Option<String>,
    pub ssh_auth_type: Option<String>,
    pub ssh_key_path: Option<String>,
    pub ssl_enabled: bool,
    pub ssl_ca_path: Option<String>,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    pub pool_max: i64,
    pub ping_interval: Option<i64>,
    pub safe_mode: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConnectionGroupRow {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QueryHistoryRow {
    pub id: String,
    pub connection_id: String,
    pub sql: String,
    pub executed_at: String,
    pub duration_us: Option<i64>,
    pub row_count: Option<i64>,
    pub error: Option<String>,
    pub status: String,
}

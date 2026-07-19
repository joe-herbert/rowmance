/// Tauri commands for dashboard management.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug, Clone)]
pub struct Dashboard {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub pinned: bool,
    #[serde(rename = "pinnedOrder")]
    pub pinned_order: Option<i64>,
    pub widgets: serde_json::Value,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Deserialize, Debug)]
pub struct DashboardCreateInput {
    pub name: String,
    pub icon: String,
}

#[derive(Deserialize, Debug)]
pub struct DashboardUpdateInput {
    pub name: String,
    pub icon: String,
    pub pinned: bool,
    #[serde(rename = "pinnedOrder")]
    pub pinned_order: Option<i64>,
    pub widgets: serde_json::Value,
}

// ── SQLite row type ───────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct DashboardRow {
    id: String,
    name: String,
    icon: String,
    pinned: bool,
    pinned_order: Option<i64>,
    widgets_json: String,
    created_at: String,
    updated_at: String,
}

impl From<DashboardRow> for Dashboard {
    fn from(r: DashboardRow) -> Self {
        let widgets =
            serde_json::from_str(&r.widgets_json).unwrap_or(serde_json::Value::Array(vec![]));
        Self {
            id: r.id,
            name: r.name,
            icon: r.icon,
            pinned: r.pinned,
            pinned_order: r.pinned_order,
            widgets,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn dashboards_list(sqlite: State<'_, SqlitePool>) -> Result<Vec<Dashboard>, AppError> {
    let rows = sqlx::query_as::<_, DashboardRow>("SELECT * FROM dashboards ORDER BY created_at")
        .fetch_all(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(Dashboard::from).collect())
}

#[tauri::command]
pub async fn dashboards_create(
    sqlite: State<'_, SqlitePool>,
    input: DashboardCreateInput,
) -> Result<Dashboard, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO dashboards (id, name, icon, pinned, pinned_order, widgets_json, created_at, updated_at)
         VALUES (?, ?, ?, 0, NULL, '[]', ?, ?)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.icon)
    .bind(&now)
    .bind(&now)
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, DashboardRow>("SELECT * FROM dashboards WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(Dashboard::from(row))
}

#[tauri::command]
pub async fn dashboards_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: DashboardUpdateInput,
) -> Result<Dashboard, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let widgets_json = serde_json::to_string(&input.widgets).unwrap_or_else(|_| "[]".to_string());
    let pinned_i = input.pinned as i64;

    sqlx::query(
        "UPDATE dashboards
         SET name = ?, icon = ?, pinned = ?, pinned_order = ?, widgets_json = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.icon)
    .bind(pinned_i)
    .bind(input.pinned_order)
    .bind(&widgets_json)
    .bind(&now)
    .bind(&id)
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, DashboardRow>("SELECT * FROM dashboards WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(Dashboard::from(row))
}

#[tauri::command]
pub async fn dashboards_delete(sqlite: State<'_, SqlitePool>, id: String) -> Result<(), AppError> {
    sqlx::query("DELETE FROM dashboards WHERE id = ?")
        .bind(&id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

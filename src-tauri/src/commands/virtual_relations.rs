/// Tauri commands for managing virtual column connections.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ColumnRef {
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub database: String,
    pub table: String,
    pub column: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct VirtualRelation {
    pub id: String,
    pub label: Option<String>,
    pub from: ColumnRef,
    pub to: ColumnRef,
}

#[derive(Deserialize, Debug)]
pub struct VirtualRelationInput {
    pub label: Option<String>,
    pub from: ColumnRef,
    pub to: ColumnRef,
}

// ── SQLite row type ───────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct VirtualRelationRow {
    id: String,
    label: Option<String>,
    from_connection_id: String,
    from_database: String,
    from_table: String,
    from_column: String,
    to_connection_id: String,
    to_database: String,
    to_table: String,
    to_column: String,
}

impl From<VirtualRelationRow> for VirtualRelation {
    fn from(r: VirtualRelationRow) -> Self {
        Self {
            id: r.id,
            label: r.label,
            from: ColumnRef {
                connection_id: r.from_connection_id,
                database: r.from_database,
                table: r.from_table,
                column: r.from_column,
            },
            to: ColumnRef {
                connection_id: r.to_connection_id,
                database: r.to_database,
                table: r.to_table,
                column: r.to_column,
            },
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn virtual_relations_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<VirtualRelation>, AppError> {
    let rows = sqlx::query_as::<_, VirtualRelationRow>(
        "SELECT * FROM virtual_relations ORDER BY created_at",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(VirtualRelation::from).collect())
}

#[tauri::command]
pub async fn virtual_relations_create(
    sqlite: State<'_, SqlitePool>,
    input: VirtualRelationInput,
) -> Result<VirtualRelation, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        INSERT INTO virtual_relations (
            id, label,
            from_connection_id, from_database, from_table, from_column,
            to_connection_id, to_database, to_table, to_column,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        id,
        input.label,
        input.from.connection_id,
        input.from.database,
        input.from.table,
        input.from.column,
        input.to.connection_id,
        input.to.database,
        input.to.table,
        input.to.column,
        now,
        now,
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, VirtualRelationRow>("SELECT * FROM virtual_relations WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(VirtualRelation::from(row))
}

#[tauri::command]
pub async fn virtual_relations_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: VirtualRelationInput,
) -> Result<VirtualRelation, AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        UPDATE virtual_relations
        SET label = ?,
            from_connection_id = ?, from_database = ?, from_table = ?, from_column = ?,
            to_connection_id = ?, to_database = ?, to_table = ?, to_column = ?,
            updated_at = ?
        WHERE id = ?
        "#,
        input.label,
        input.from.connection_id,
        input.from.database,
        input.from.table,
        input.from.column,
        input.to.connection_id,
        input.to.database,
        input.to.table,
        input.to.column,
        now,
        id,
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, VirtualRelationRow>("SELECT * FROM virtual_relations WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(VirtualRelation::from(row))
}

#[tauri::command]
pub async fn virtual_relations_delete(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM virtual_relations WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

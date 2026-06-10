/// Tauri commands for query history.
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::db::models::QueryHistoryRow;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct QueryHistoryEntry {
    pub id: String,
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub sql: String,
    #[serde(rename = "executedAt")]
    pub executed_at: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<i64>,
    #[serde(rename = "rowCount")]
    pub row_count: Option<i64>,
    pub error: Option<String>,
    pub status: String,
}

impl From<QueryHistoryRow> for QueryHistoryEntry {
    fn from(r: QueryHistoryRow) -> Self {
        Self {
            id: r.id,
            connection_id: r.connection_id,
            sql: r.sql,
            executed_at: r.executed_at,
            duration_ms: r.duration_ms,
            row_count: r.row_count,
            error: r.error,
            status: r.status,
        }
    }
}

/// List recent query history for a connection.
#[tauri::command]
pub async fn history_list(
    sqlite: State<'_, SqlitePool>,
    connection_id: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<QueryHistoryEntry>, AppError> {
    let rows = sqlx::query_as::<_, QueryHistoryRow>(
        "SELECT * FROM query_history WHERE connection_id = ? ORDER BY executed_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&connection_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(QueryHistoryEntry::from).collect())
}

/// Clear all history for a connection.
#[tauri::command]
pub async fn history_clear(
    sqlite: State<'_, SqlitePool>,
    connection_id: String,
) -> Result<(), AppError> {
    sqlx::query!(
        "DELETE FROM query_history WHERE connection_id = ?",
        connection_id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

/// Delete a single history entry.
#[tauri::command]
pub async fn history_delete(sqlite: State<'_, SqlitePool>, id: String) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM query_history WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations").run(&pool).await.unwrap();
        pool
    }

    /// Insert a minimal connection profile so history FK constraints are satisfied.
    async fn insert_profile(pool: &SqlitePool, id: &str) {
        sqlx::query(
            r#"INSERT INTO connection_profiles
               (id, name, db_type, host, port, database, username, read_only,
                ssh_enabled, ssl_enabled, pool_min, pool_max, created_at, updated_at)
               VALUES (?, 'test', 'postgres', 'localhost', 5432, 'db', 'user', 0,
                       0, 0, 1, 5, '2024-01-01', '2024-01-01')"#,
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_history(pool: &SqlitePool, id: &str, conn_id: &str, sql: &str, status: &str) {
        sqlx::query(
            r#"INSERT INTO query_history (id, connection_id, sql, executed_at, status)
               VALUES (?, ?, ?, datetime('now'), ?)"#,
        )
        .bind(id)
        .bind(conn_id)
        .bind(sql)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn list_returns_empty_for_new_connection() {
        let pool = setup_db().await;
        let rows = sqlx::query_as::<_, QueryHistoryRow>(
            "SELECT * FROM query_history WHERE connection_id = ? ORDER BY executed_at DESC LIMIT 100 OFFSET 0",
        )
        .bind("no-such-conn")
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn list_returns_entries_for_connection() {
        let pool = setup_db().await;
        insert_profile(&pool, "c-1").await;
        insert_history(&pool, "h-1", "c-1", "SELECT 1", "success").await;
        insert_history(&pool, "h-2", "c-1", "SELECT 2", "success").await;

        let rows = sqlx::query_as::<_, QueryHistoryRow>(
            "SELECT * FROM query_history WHERE connection_id = ? ORDER BY executed_at DESC LIMIT 100 OFFSET 0",
        )
        .bind("c-1")
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn list_does_not_return_entries_for_other_connections() {
        let pool = setup_db().await;
        insert_profile(&pool, "c-a").await;
        insert_profile(&pool, "c-b").await;
        insert_history(&pool, "h-a", "c-a", "SELECT 1", "success").await;

        let rows = sqlx::query_as::<_, QueryHistoryRow>(
            "SELECT * FROM query_history WHERE connection_id = ? ORDER BY executed_at DESC LIMIT 100 OFFSET 0",
        )
        .bind("c-b")
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn clear_removes_all_entries_for_connection() {
        let pool = setup_db().await;
        insert_profile(&pool, "c-2").await;
        insert_history(&pool, "h-3", "c-2", "SELECT 1", "success").await;
        insert_history(&pool, "h-4", "c-2", "SELECT 2", "error").await;

        sqlx::query("DELETE FROM query_history WHERE connection_id = ?")
            .bind("c-2")
            .execute(&pool)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM query_history WHERE connection_id = 'c-2'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn delete_removes_a_single_entry() {
        let pool = setup_db().await;
        insert_profile(&pool, "c-3").await;
        insert_history(&pool, "h-5", "c-3", "SELECT 1", "success").await;
        insert_history(&pool, "h-6", "c-3", "SELECT 2", "success").await;

        sqlx::query("DELETE FROM query_history WHERE id = ?")
            .bind("h-5")
            .execute(&pool)
            .await
            .unwrap();

        let rows = sqlx::query_as::<_, QueryHistoryRow>(
            "SELECT * FROM query_history WHERE connection_id = ?",
        )
        .bind("c-3")
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "h-6");
    }

    #[tokio::test]
    async fn history_respects_limit_and_offset() {
        let pool = setup_db().await;
        insert_profile(&pool, "c-4").await;
        for i in 0..5 {
            insert_history(&pool, &format!("h-pg-{i}"), "c-4", &format!("SELECT {i}"), "success").await;
        }

        let page1 = sqlx::query_as::<_, QueryHistoryRow>(
            "SELECT * FROM query_history WHERE connection_id = ? ORDER BY executed_at DESC LIMIT 2 OFFSET 0",
        )
        .bind("c-4")
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = sqlx::query_as::<_, QueryHistoryRow>(
            "SELECT * FROM query_history WHERE connection_id = ? ORDER BY executed_at DESC LIMIT 2 OFFSET 2",
        )
        .bind("c-4")
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(page2.len(), 2);

        // Pages must not overlap.
        let ids1: Vec<_> = page1.iter().map(|r| r.id.clone()).collect();
        let ids2: Vec<_> = page2.iter().map(|r| r.id.clone()).collect();
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
    }
}

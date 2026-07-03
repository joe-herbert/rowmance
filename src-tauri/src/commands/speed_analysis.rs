/// Tauri commands for query speed analysis (dev-only).
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct SpeedAnalysisEntry {
    pub id: String,
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub sql: String,
    #[serde(rename = "executedAt")]
    pub executed_at: String,
    #[serde(rename = "totalUs")]
    pub total_us: i64,
    #[serde(rename = "poolAcquireUs")]
    pub pool_acquire_us: i64,
    #[serde(rename = "dbSwitchUs")]
    pub db_switch_us: i64,
    #[serde(rename = "executionUs")]
    pub execution_us: i64,
    #[serde(rename = "resultProcessingUs")]
    pub result_processing_us: i64,
    #[serde(rename = "rowCount")]
    pub row_count: Option<i64>,
}

#[tauri::command]
pub async fn speed_analysis_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<SpeedAnalysisEntry>, AppError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<i64>,
        ),
    >(
        r#"SELECT id, connection_id, sql, executed_at, total_us, pool_acquire_us,
           db_switch_us, execution_us, result_processing_us, row_count
           FROM query_speed_analysis ORDER BY executed_at DESC"#,
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                connection_id,
                sql,
                executed_at,
                total_us,
                pool_acquire_us,
                db_switch_us,
                execution_us,
                result_processing_us,
                row_count,
            )| SpeedAnalysisEntry {
                id,
                connection_id,
                sql,
                executed_at,
                total_us,
                pool_acquire_us,
                db_switch_us,
                execution_us,
                result_processing_us,
                row_count,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn speed_analysis_clear(sqlite: State<'_, SqlitePool>) -> Result<(), AppError> {
    sqlx::query("DELETE FROM query_speed_analysis")
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

/// Tauri commands for schema introspection.
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, State};

use crate::connections::pool_manager::ConnectionManager;
use crate::connections::types::{BulkColumnRow, ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo};
use crate::error::AppError;

#[derive(Clone, Serialize)]
struct TableCountPayload {
    #[serde(rename = "connectionId")]
    connection_id: String,
    database: String,
    #[serde(rename = "tableName")]
    table_name: String,
    count: i64,
}

/// List all databases visible to this connection.
#[tauri::command]
pub async fn schema_list_databases(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<String>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_databases().await.map_err(AppError::from)
}

/// List all tables in the given database/schema.
/// Row counts are returned immediately using cheap estimates where available.
/// Accurate counts for tables with a zero/null estimate are fetched in the
/// background and pushed via the `table-count-updated` Tauri event.
#[tauri::command]
pub async fn schema_list_tables(
    app: tauri::AppHandle,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
) -> Result<Vec<TableInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    let tables = engine.list_tables(&database).await.map_err(AppError::from)?;

    let names: Vec<String> = tables
        .iter()
        .filter(|t| t.table_type == "table" && matches!(t.row_count, Some(0) | None))
        .map(|t| t.name.clone())
        .collect();
    if !names.is_empty() {
        let engine = engine.clone();
        let conn_id = connection_id.clone();
        let db = database.clone();
        tokio::spawn(async move {
            for name in names {
                if let Ok(count) = engine.count_table(&db, &name).await {
                    let _ = app.emit(
                        "table-count-updated",
                        TableCountPayload {
                            connection_id: conn_id.clone(),
                            database: db.clone(),
                            table_name: name,
                            count,
                        },
                    );
                }
            }
        });
    }

    Ok(tables)
}

/// List all columns in the given table.
#[tauri::command]
pub async fn schema_list_columns(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<Vec<ColumnInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_columns(&database, &table).await.map_err(AppError::from)
}

/// List all columns for every table and view in a database in one round-trip.
/// Used by global search to populate column data without N separate IPC calls.
#[tauri::command]
pub async fn schema_list_all_columns(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
) -> Result<Vec<BulkColumnRow>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_all_columns(&database).await.map_err(AppError::from)
}

/// List all indexes for a given table.
#[tauri::command]
pub async fn schema_list_indexes(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<Vec<IndexInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_indexes(&database, &table).await.map_err(AppError::from)
}

/// List all foreign keys for a given table.
#[tauri::command]
pub async fn schema_list_foreign_keys(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<Vec<ForeignKeyInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_foreign_keys(&database, &table).await.map_err(AppError::from)
}

/// Execute a DDL statement (ALTER TABLE, CREATE INDEX, etc.) against the connection.
/// The caller is responsible for generating correct, database-specific SQL.
#[tauri::command]
pub async fn schema_execute_ddl(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
) -> Result<(), AppError> {
    let profile_row = sqlx::query!(
        "SELECT read_only FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    match profile_row {
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ))
        }
        Some(row) if row.read_only != 0 => {
            return Err(AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode — DDL statements are not allowed",
            ));
        }
        _ => {}
    }

    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.execute_ddl(&sql).await.map_err(AppError::from)
}

/// Return the DDL for a table or view.
#[tauri::command]
pub async fn schema_get_ddl(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    object_name: String,
    #[allow(unused_variables)] object_type: String,
) -> Result<String, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.get_ddl(&database, &object_name).await.map_err(AppError::from)
}

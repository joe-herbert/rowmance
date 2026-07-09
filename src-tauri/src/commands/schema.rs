/// Tauri commands for schema introspection.
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, State};

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::{AppError, RowmanceError};

#[derive(Clone, Serialize)]
struct TableCountPayload {
    #[serde(rename = "connectionId")]
    connection_id: String,
    database: String,
    #[serde(rename = "tableName")]
    table_name: String,
    count: i64,
}

#[derive(Debug, Serialize)]
pub struct TableInfo {
    pub name: String,
    #[serde(rename = "tableType")]
    pub table_type: String,
    #[serde(rename = "rowCount")]
    pub row_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
    #[serde(rename = "isAutoIncrement")]
    pub is_auto_increment: bool,
    #[serde(rename = "isForeignKey")]
    pub is_foreign_key: bool,
    pub comment: Option<String>,
}

/// List all databases visible to this connection.
#[tauri::command]
pub async fn schema_list_databases(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<String>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => crate::connections::mysql::list_databases(pool)
            .await
            .map_err(AppError::from),
        RemotePool::Postgres(pool) => crate::connections::postgres::list_databases(pool)
            .await
            .map_err(AppError::from),
        RemotePool::Sqlite(pool) => crate::connections::sqlite::list_databases(pool)
            .await
            .map_err(AppError::from),
    }
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
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            let tables = crate::connections::mysql::list_tables(pool, &database)
                .await
                .map_err(AppError::from)?;
            let result: Vec<TableInfo> = tables
                .iter()
                .map(|t| TableInfo {
                    name: t.name.clone(),
                    table_type: t.table_type.clone(),
                    row_count: t.row_count,
                })
                .collect();
            let names: Vec<String> = tables
                .iter()
                .filter(|t| t.table_type == "table" && matches!(t.row_count, Some(0) | None))
                .map(|t| t.name.clone())
                .collect();
            if !names.is_empty() {
                let pool = pool.clone();
                let conn_id = connection_id.clone();
                let db = database.clone();
                // Sequential: at most one pool connection used for background counts,
                // leaving the rest available for user-initiated queries.
                tokio::spawn(async move {
                    for name in names {
                        if let Ok(count) =
                            crate::connections::mysql::count_table(&pool, &db, &name).await
                        {
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
            Ok(result)
        }
        RemotePool::Postgres(pool) => {
            let tables = crate::connections::postgres::list_tables(pool, &database)
                .await
                .map_err(AppError::from)?;
            let result: Vec<TableInfo> = tables
                .iter()
                .map(|t| TableInfo {
                    name: t.name.clone(),
                    table_type: t.table_type.clone(),
                    row_count: t.row_count,
                })
                .collect();
            let names: Vec<String> = tables
                .iter()
                .filter(|t| t.table_type == "table" && matches!(t.row_count, Some(0) | None))
                .map(|t| t.name.clone())
                .collect();
            if !names.is_empty() {
                let pool = pool.clone();
                let conn_id = connection_id.clone();
                let db = database.clone();
                tokio::spawn(async move {
                    for name in names {
                        if let Ok(count) =
                            crate::connections::postgres::count_table(&pool, &db, &name).await
                        {
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
            Ok(result)
        }
        RemotePool::Sqlite(pool) => {
            let tables = crate::connections::sqlite::list_tables(pool, &database)
                .await
                .map_err(AppError::from)?;
            let result: Vec<TableInfo> = tables
                .iter()
                .map(|t| TableInfo {
                    name: t.name.clone(),
                    table_type: t.table_type.clone(),
                    row_count: t.row_count,
                })
                .collect();
            // SQLite tables always start with row_count: None — count them all.
            let names: Vec<String> = tables
                .iter()
                .filter(|t| t.table_type == "table")
                .map(|t| t.name.clone())
                .collect();
            if !names.is_empty() {
                let pool = pool.clone();
                let conn_id = connection_id.clone();
                let db = database.clone();
                tokio::spawn(async move {
                    for name in names {
                        if let Ok(count) =
                            crate::connections::sqlite::count_table(&pool, &name).await
                        {
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
            Ok(result)
        }
    }
}

/// List all columns in the given table.
#[tauri::command]
pub async fn schema_list_columns(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<Vec<ColumnInfo>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            let cols = crate::connections::mysql::list_columns(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(cols
                .into_iter()
                .map(|c| ColumnInfo {
                    name: c.name,
                    data_type: c.data_type,
                    nullable: c.nullable,
                    default_value: c.default_value,
                    is_primary_key: c.is_primary_key,
                    is_auto_increment: c.is_auto_increment,
                    is_foreign_key: c.is_foreign_key,
                    comment: c.comment,
                })
                .collect())
        }
        RemotePool::Postgres(pool) => {
            let cols = crate::connections::postgres::list_columns(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(cols
                .into_iter()
                .map(|c| ColumnInfo {
                    name: c.name,
                    data_type: c.data_type,
                    nullable: c.nullable,
                    default_value: c.default_value,
                    is_primary_key: c.is_primary_key,
                    is_auto_increment: c.is_auto_increment,
                    is_foreign_key: c.is_foreign_key,
                    comment: c.comment,
                })
                .collect())
        }
        RemotePool::Sqlite(pool) => {
            let cols = crate::connections::sqlite::list_columns(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(cols
                .into_iter()
                .map(|c| ColumnInfo {
                    name: c.name,
                    data_type: c.data_type,
                    nullable: c.nullable,
                    default_value: c.default_value,
                    is_primary_key: c.is_primary_key,
                    is_auto_increment: c.is_auto_increment,
                    is_foreign_key: c.is_foreign_key,
                    comment: c.comment,
                })
                .collect())
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BulkColumnRow {
    #[serde(rename = "tableName")]
    pub table_name: String,
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
    #[serde(rename = "isAutoIncrement")]
    pub is_auto_increment: bool,
    #[serde(rename = "isForeignKey")]
    pub is_foreign_key: bool,
    pub comment: Option<String>,
}

/// List all columns for every table and view in a database in one round-trip.
/// Used by global search to populate column data without N separate IPC calls.
#[tauri::command]
pub async fn schema_list_all_columns(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
) -> Result<Vec<BulkColumnRow>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    macro_rules! to_bulk {
        ($pairs:expr) => {
            $pairs
                .into_iter()
                .map(|(table_name, col)| BulkColumnRow {
                    table_name,
                    name: col.name,
                    data_type: col.data_type,
                    nullable: col.nullable,
                    default_value: col.default_value,
                    is_primary_key: col.is_primary_key,
                    is_auto_increment: col.is_auto_increment,
                    is_foreign_key: col.is_foreign_key,
                    comment: col.comment,
                })
                .collect::<Vec<_>>()
        };
    }
    let rows = match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            to_bulk!(crate::connections::mysql::list_all_columns(pool, &database)
                .await
                .map_err(AppError::from)?)
        }
        RemotePool::Postgres(pool) => to_bulk!(crate::connections::postgres::list_all_columns(
            pool, &database
        )
        .await
        .map_err(AppError::from)?),
        RemotePool::Sqlite(pool) => to_bulk!(crate::connections::sqlite::list_all_columns(
            pool, &database
        )
        .await
        .map_err(AppError::from)?),
    };
    Ok(rows)
}

#[derive(Debug, Serialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    #[serde(rename = "indexType")]
    pub index_type: String,
}

#[derive(Debug, Serialize)]
pub struct ForeignKeyInfo {
    #[serde(rename = "constraintName")]
    pub constraint_name: String,
    pub columns: Vec<String>,
    #[serde(rename = "referencedTable")]
    pub referenced_table: String,
    #[serde(rename = "referencedColumns")]
    pub referenced_columns: Vec<String>,
    #[serde(rename = "onDelete")]
    pub on_delete: String,
    #[serde(rename = "onUpdate")]
    pub on_update: String,
}

/// List all indexes for a given table.
#[tauri::command]
pub async fn schema_list_indexes(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<Vec<IndexInfo>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            let rows = crate::connections::mysql::list_indexes(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(rows
                .into_iter()
                .map(|r| IndexInfo {
                    name: r.name,
                    columns: r.columns,
                    unique: r.unique,
                    index_type: r.index_type,
                })
                .collect())
        }
        RemotePool::Postgres(pool) => {
            let rows = crate::connections::postgres::list_indexes(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(rows
                .into_iter()
                .map(|r| IndexInfo {
                    name: r.name,
                    columns: r.columns,
                    unique: r.unique,
                    index_type: r.index_type,
                })
                .collect())
        }
        RemotePool::Sqlite(pool) => {
            let rows = crate::connections::sqlite::list_indexes(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(rows
                .into_iter()
                .map(|r| IndexInfo {
                    name: r.name,
                    columns: r.columns,
                    unique: r.unique,
                    index_type: r.index_type,
                })
                .collect())
        }
    }
}

/// List all foreign keys for a given table.
#[tauri::command]
pub async fn schema_list_foreign_keys(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
) -> Result<Vec<ForeignKeyInfo>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            let rows = crate::connections::mysql::list_foreign_keys(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(rows
                .into_iter()
                .map(|r| ForeignKeyInfo {
                    constraint_name: r.constraint_name,
                    columns: r.columns,
                    referenced_table: r.referenced_table,
                    referenced_columns: r.referenced_columns,
                    on_delete: r.on_delete,
                    on_update: r.on_update,
                })
                .collect())
        }
        RemotePool::Postgres(pool) => {
            let rows = crate::connections::postgres::list_foreign_keys(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(rows
                .into_iter()
                .map(|r| ForeignKeyInfo {
                    constraint_name: r.constraint_name,
                    columns: r.columns,
                    referenced_table: r.referenced_table,
                    referenced_columns: r.referenced_columns,
                    on_delete: r.on_delete,
                    on_update: r.on_update,
                })
                .collect())
        }
        RemotePool::Sqlite(pool) => {
            let rows = crate::connections::sqlite::list_foreign_keys(pool, &database, &table)
                .await
                .map_err(AppError::from)?;
            Ok(rows
                .into_iter()
                .map(|r| ForeignKeyInfo {
                    constraint_name: r.constraint_name,
                    columns: r.columns,
                    referenced_table: r.referenced_table,
                    referenced_columns: r.referenced_columns,
                    on_delete: r.on_delete,
                    on_update: r.on_update,
                })
                .collect())
        }
    }
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

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => sqlx::query(&sql)
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| AppError::from(RowmanceError::Database(e)))?,
        RemotePool::Postgres(pool) => sqlx::query(&sql)
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| AppError::from(RowmanceError::Database(e)))?,
        RemotePool::Sqlite(pool) => sqlx::query(&sql)
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| AppError::from(RowmanceError::Database(e)))?,
    }
    Ok(())
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
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => crate::connections::mysql::get_ddl(pool, &database, &object_name)
            .await
            .map_err(AppError::from),
        RemotePool::Postgres(pool) => {
            crate::connections::postgres::get_ddl(pool, &database, &object_name)
                .await
                .map_err(AppError::from)
        }
        RemotePool::Sqlite(pool) => crate::connections::sqlite::get_ddl(pool, &object_name)
            .await
            .map_err(AppError::from),
    }
}

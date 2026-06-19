/// Tauri commands for schema introspection.
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;

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
        RemotePool::MySql(pool) => crate::connections::mysql::list_databases(pool)
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
#[tauri::command]
pub async fn schema_list_tables(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
) -> Result<Vec<TableInfo>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let tables = crate::connections::mysql::list_tables(pool, &database)
                .await
                .map_err(AppError::from)?;
            Ok(tables
                .into_iter()
                .map(|t| TableInfo {
                    name: t.name,
                    table_type: t.table_type,
                    row_count: t.row_count,
                })
                .collect())
        }
        RemotePool::Postgres(pool) => {
            let tables = crate::connections::postgres::list_tables(pool, &database)
                .await
                .map_err(AppError::from)?;
            Ok(tables
                .into_iter()
                .map(|t| TableInfo {
                    name: t.name,
                    table_type: t.table_type,
                    row_count: t.row_count,
                })
                .collect())
        }
        RemotePool::Sqlite(pool) => {
            let tables = crate::connections::sqlite::list_tables(pool, &database)
                .await
                .map_err(AppError::from)?;
            Ok(tables
                .into_iter()
                .map(|t| TableInfo {
                    name: t.name,
                    table_type: t.table_type,
                    row_count: t.row_count,
                })
                .collect())
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
        RemotePool::MySql(pool) => {
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
        RemotePool::MySql(pool) => {
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
        RemotePool::MySql(pool) => {
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
        RemotePool::MySql(pool) => crate::connections::mysql::get_ddl(pool, &object_name)
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

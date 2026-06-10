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
    }
}

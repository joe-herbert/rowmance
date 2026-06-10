/// MySQL/MariaDB-specific schema introspection queries.
use serde::Serialize;
use sqlx::MySqlPool;

use crate::error::RowmanceError;

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
pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<String>, RowmanceError> {
    let rows = sqlx::query_scalar::<_, String>("SHOW DATABASES")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// List all tables (and views) in the given database.
pub async fn list_tables(
    pool: &MySqlPool,
    database: &str,
) -> Result<Vec<TableInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: Option<String>,
        table_type: Option<String>,
        row_count: Option<u64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            TABLE_NAME AS name,
            TABLE_TYPE AS table_type,
            TABLE_ROWS AS row_count
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = ?
        ORDER BY TABLE_NAME
        "#,
    )
    .bind(database)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TableInfo {
            name: r.name.unwrap_or_default(),
            table_type: if r.table_type.as_deref() == Some("VIEW") {
                "view".to_owned()
            } else {
                "table".to_owned()
            },
            row_count: r.row_count.map(|v| v as i64),
        })
        .collect())
}

/// List all columns for a given table.
pub async fn list_columns(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: Option<String>,
        data_type: Option<String>,
        nullable: Option<String>,
        default_value: Option<String>,
        column_key: Option<String>,
        extra: Option<String>,
        comment: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            c.COLUMN_NAME    AS name,
            c.COLUMN_TYPE    AS data_type,
            c.IS_NULLABLE    AS nullable,
            c.COLUMN_DEFAULT AS default_value,
            c.COLUMN_KEY     AS column_key,
            c.EXTRA          AS extra,
            c.COLUMN_COMMENT AS comment
        FROM information_schema.COLUMNS c
        WHERE c.TABLE_SCHEMA = ? AND c.TABLE_NAME = ?
        ORDER BY c.ORDINAL_POSITION
        "#,
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let is_pk = r.column_key.as_deref() == Some("PRI");
            let is_fk = r.column_key.as_deref() == Some("MUL");
            let is_auto = r
                .extra
                .as_deref()
                .map(|e| e.contains("auto_increment"))
                .unwrap_or(false);
            ColumnInfo {
                name: r.name.unwrap_or_default(),
                data_type: r.data_type.unwrap_or_default(),
                nullable: r.nullable.as_deref() == Some("YES"),
                default_value: r.default_value,
                is_primary_key: is_pk,
                is_auto_increment: is_auto,
                is_foreign_key: is_fk,
                comment: if r.comment.as_deref() == Some("") {
                    None
                } else {
                    r.comment
                },
            }
        })
        .collect())
}

/// Return the CREATE TABLE / CREATE VIEW DDL for an object.
pub async fn get_ddl(pool: &MySqlPool, table: &str) -> Result<String, RowmanceError> {
    // SHOW CREATE TABLE works for both tables and views in MySQL/MariaDB.
    let row = sqlx::query(&format!("SHOW CREATE TABLE `{table}`"))
        .fetch_one(pool)
        .await?;

    // The DDL is in the second column; column name varies (Create Table / Create View).
    use sqlx::Row;
    let ddl: String = row.try_get(1).unwrap_or_default();
    Ok(ddl)
}

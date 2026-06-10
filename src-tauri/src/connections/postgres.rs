/// PostgreSQL-specific schema introspection queries.
use serde::Serialize;
use sqlx::PgPool;

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

/// List all non-system databases visible to this connection.
pub async fn list_databases(pool: &PgPool) -> Result<Vec<String>, RowmanceError> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// List all user tables and views in the given schema (defaults to 'public').
pub async fn list_tables(pool: &PgPool, schema: &str) -> Result<Vec<TableInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        table_name: Option<String>,
        table_type: Option<String>,
        row_count: Option<i64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            t.table_name,
            t.table_type,
            s.n_live_tup::bigint AS row_count
        FROM information_schema.tables t
        LEFT JOIN pg_stat_user_tables s
               ON s.schemaname = t.table_schema
              AND s.relname = t.table_name
        WHERE t.table_schema = $1
          AND t.table_type IN ('BASE TABLE', 'VIEW')
        ORDER BY t.table_name
        "#,
    )
    .bind(schema)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TableInfo {
            name: r.table_name.unwrap_or_default(),
            table_type: if r.table_type.as_deref() == Some("VIEW") {
                "view".to_owned()
            } else {
                "table".to_owned()
            },
            row_count: r.row_count,
        })
        .collect())
}

/// List all columns for a given table in the given schema.
pub async fn list_columns(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        column_name: Option<String>,
        data_type: Option<String>,
        nullable: Option<bool>,
        column_default: Option<String>,
        is_primary_key: Option<bool>,
        is_foreign_key: Option<bool>,
        comment: Option<String>,
    }

    // Join with pg_constraint to determine primary key and foreign key status.
    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            c.column_name,
            c.udt_name AS data_type,
            (c.is_nullable = 'YES') AS nullable,
            c.column_default,
            EXISTS (
                SELECT 1
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                  ON kcu.constraint_name = tc.constraint_name
                 AND kcu.table_schema = tc.table_schema
                WHERE tc.constraint_type = 'PRIMARY KEY'
                  AND kcu.table_schema = $1
                  AND kcu.table_name   = $2
                  AND kcu.column_name  = c.column_name
            ) AS is_primary_key,
            EXISTS (
                SELECT 1
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                  ON kcu.constraint_name = tc.constraint_name
                 AND kcu.table_schema = tc.table_schema
                WHERE tc.constraint_type = 'FOREIGN KEY'
                  AND kcu.table_schema = $1
                  AND kcu.table_name   = $2
                  AND kcu.column_name  = c.column_name
            ) AS is_foreign_key,
            col_description(
                (quote_ident($1) || '.' || quote_ident($2))::regclass::oid,
                c.ordinal_position
            ) AS comment
        FROM information_schema.columns c
        WHERE c.table_schema = $1 AND c.table_name = $2
        ORDER BY c.ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let is_serial = r
                .column_default
                .as_deref()
                .map(|d| d.starts_with("nextval("))
                .unwrap_or(false);
            ColumnInfo {
                name: r.column_name.unwrap_or_default(),
                data_type: r.data_type.unwrap_or_default(),
                nullable: r.nullable.unwrap_or(true),
                default_value: r.column_default,
                is_primary_key: r.is_primary_key.unwrap_or(false),
                is_auto_increment: is_serial,
                is_foreign_key: r.is_foreign_key.unwrap_or(false),
                comment: r.comment,
            }
        })
        .collect())
}

/// Return the DDL for a table or view using pg_get_tabledef-compatible approach.
pub async fn get_ddl(pool: &PgPool, schema: &str, table: &str) -> Result<String, RowmanceError> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT pg_get_tabledef($1::text, $2::text, false, 'FKEYS_INTERNAL')",
    )
    .bind(schema)
    .bind(table)
    .fetch_optional(pool)
    .await;

    // pg_get_tabledef is not available in all Postgres versions; fall back to a
    // simpler column listing if the function does not exist.
    match row {
        Ok(Some(ddl)) => Ok(ddl),
        _ => {
            let cols = list_columns(pool, schema, table).await?;
            let col_lines: Vec<String> = cols
                .iter()
                .map(|c| format!("  {} {}", c.name, c.data_type))
                .collect();
            Ok(format!(
                "-- DDL not available; column listing:\nCREATE TABLE {}.{} (\n{}\n);",
                schema,
                table,
                col_lines.join(",\n")
            ))
        }
    }
}

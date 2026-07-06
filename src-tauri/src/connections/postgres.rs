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
/// Uses pg_catalog directly — faster than information_schema.tables.
pub async fn list_tables(pool: &PgPool, schema: &str) -> Result<Vec<TableInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        table_name: Option<String>,
        relkind: Option<i8>,
        row_count: Option<i64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            cl.relname              AS table_name,
            cl.relkind              AS relkind,
            s.n_live_tup::bigint    AS row_count
        FROM pg_catalog.pg_class cl
        JOIN pg_catalog.pg_namespace n ON n.oid = cl.relnamespace
        LEFT JOIN pg_stat_user_tables s
               ON s.schemaname = n.nspname
              AND s.relname    = cl.relname
        WHERE n.nspname = $1
          AND cl.relkind IN ('r', 'v', 'm', 'p')
        ORDER BY cl.relname
        "#,
    )
    .bind(schema)
    .fetch_all(pool)
    .await?;

    let tables: Vec<TableInfo> = rows
        .into_iter()
        .map(|r| {
            let kind = r.relkind.unwrap_or(b'r' as i8) as u8 as char;
            TableInfo {
                name: r.table_name.unwrap_or_default(),
                table_type: if kind == 'v' || kind == 'm' {
                    "view".to_owned()
                } else {
                    "table".to_owned()
                },
                row_count: r.row_count,
            }
        })
        .collect();

    Ok(tables)
}

/// Count all rows in a table. Used by the background count task.
pub async fn count_table(pool: &PgPool, schema: &str, table: &str) -> Result<i64, RowmanceError> {
    let schema_esc = schema.replace('"', "\"\"");
    let tbl_esc = table.replace('"', "\"\"");
    let count: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM \"{}\".\"{}\"",
        schema_esc, tbl_esc
    ))
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// List all columns for a given table in the given schema.
/// Uses pg_catalog directly — avoids the slow information_schema views.
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

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            a.attname                                   AS column_name,
            pt.typname                                  AS data_type,
            NOT a.attnotnull                            AS nullable,
            pg_get_expr(d.adbin, d.adrelid)            AS column_default,
            EXISTS (
                SELECT 1 FROM pg_index i
                WHERE i.indrelid = a.attrelid
                  AND a.attnum   = ANY(i.indkey)
                  AND i.indisprimary
            )                                           AS is_primary_key,
            EXISTS (
                SELECT 1 FROM pg_constraint con
                WHERE con.conrelid = a.attrelid
                  AND a.attnum     = ANY(con.conkey)
                  AND con.contype  = 'f'
            )                                           AS is_foreign_key,
            col_description(a.attrelid, a.attnum)       AS comment
        FROM pg_catalog.pg_class cl
        JOIN pg_catalog.pg_namespace n    ON n.oid  = cl.relnamespace
        JOIN pg_catalog.pg_attribute a    ON a.attrelid = cl.oid
                                         AND a.attnum  > 0
                                         AND NOT a.attisdropped
        JOIN pg_catalog.pg_type pt        ON pt.oid = a.atttypid
        LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid
                                         AND d.adnum   = a.attnum
        WHERE n.nspname  = $1
          AND cl.relname = $2
          AND cl.relkind IN ('r', 'v', 'm', 'p')
        ORDER BY a.attnum
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

/// List all columns for every user table and view in the given schema in one query.
/// Uses pg_catalog directly — avoids the slow information_schema views.
/// Returns (table_name, ColumnInfo) pairs ordered by table then ordinal position.
pub async fn list_all_columns(
    pool: &PgPool,
    schema: &str,
) -> Result<Vec<(String, ColumnInfo)>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        table_name: Option<String>,
        column_name: Option<String>,
        data_type: Option<String>,
        nullable: Option<bool>,
        column_default: Option<String>,
        is_primary_key: Option<bool>,
        is_foreign_key: Option<bool>,
        comment: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            cl.relname                                  AS table_name,
            a.attname                                   AS column_name,
            pt.typname                                  AS data_type,
            NOT a.attnotnull                            AS nullable,
            pg_get_expr(d.adbin, d.adrelid)            AS column_default,
            EXISTS (
                SELECT 1 FROM pg_index i
                WHERE i.indrelid = a.attrelid
                  AND a.attnum   = ANY(i.indkey)
                  AND i.indisprimary
            )                                           AS is_primary_key,
            EXISTS (
                SELECT 1 FROM pg_constraint con
                WHERE con.conrelid = a.attrelid
                  AND a.attnum     = ANY(con.conkey)
                  AND con.contype  = 'f'
            )                                           AS is_foreign_key,
            col_description(a.attrelid, a.attnum)       AS comment
        FROM pg_catalog.pg_class cl
        JOIN pg_catalog.pg_namespace n    ON n.oid  = cl.relnamespace
        JOIN pg_catalog.pg_attribute a    ON a.attrelid = cl.oid
                                         AND a.attnum  > 0
                                         AND NOT a.attisdropped
        JOIN pg_catalog.pg_type pt        ON pt.oid = a.atttypid
        LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid
                                         AND d.adnum   = a.attnum
        WHERE n.nspname  = $1
          AND cl.relkind IN ('r', 'v', 'm', 'p')
        ORDER BY cl.relname, a.attnum
        "#,
    )
    .bind(schema)
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
            (
                r.table_name.unwrap_or_default(),
                ColumnInfo {
                    name: r.column_name.unwrap_or_default(),
                    data_type: r.data_type.unwrap_or_default(),
                    nullable: r.nullable.unwrap_or(true),
                    default_value: r.column_default,
                    is_primary_key: r.is_primary_key.unwrap_or(false),
                    is_auto_increment: is_serial,
                    is_foreign_key: r.is_foreign_key.unwrap_or(false),
                    comment: r.comment,
                },
            )
        })
        .collect())
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

/// List all indexes for a given table in the given schema.
pub async fn list_indexes(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        index_name: Option<String>,
        column_name: Option<String>,
        is_unique: Option<bool>,
        index_type: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            i.relname AS index_name,
            a.attname AS column_name,
            ix.indisunique AS is_unique,
            am.amname AS index_type
        FROM pg_index ix
        JOIN pg_class t  ON t.oid  = ix.indrelid
        JOIN pg_class i  ON i.oid  = ix.indexrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_am am    ON am.oid = i.relam
        JOIN pg_attribute a ON a.attrelid = t.oid
            AND a.attnum = ANY(ix.indkey)
        WHERE n.nspname = $1 AND t.relname = $2
        ORDER BY i.relname, array_position(ix.indkey, a.attnum)
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    let mut map: std::collections::BTreeMap<String, IndexInfo> = std::collections::BTreeMap::new();
    for r in rows {
        let name = r.index_name.unwrap_or_default();
        let col = r.column_name.unwrap_or_default();
        let entry = map.entry(name.clone()).or_insert_with(|| IndexInfo {
            name: name.clone(),
            columns: vec![],
            unique: r.is_unique.unwrap_or(false),
            index_type: r.index_type.unwrap_or_else(|| "btree".to_owned()),
        });
        entry.columns.push(col);
    }
    Ok(map.into_values().collect())
}

/// List all foreign keys for a given table in the given schema.
pub async fn list_foreign_keys(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        constraint_name: Option<String>,
        column_name: Option<String>,
        referenced_table: Option<String>,
        referenced_column: Option<String>,
        on_delete: Option<String>,
        on_update: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            c.conname AS constraint_name,
            a.attname AS column_name,
            rt.relname AS referenced_table,
            ra.attname AS referenced_column,
            CASE c.confdeltype
                WHEN 'a' THEN 'NO ACTION'
                WHEN 'r' THEN 'RESTRICT'
                WHEN 'c' THEN 'CASCADE'
                WHEN 'n' THEN 'SET NULL'
                WHEN 'd' THEN 'SET DEFAULT'
            END AS on_delete,
            CASE c.confupdtype
                WHEN 'a' THEN 'NO ACTION'
                WHEN 'r' THEN 'RESTRICT'
                WHEN 'c' THEN 'CASCADE'
                WHEN 'n' THEN 'SET NULL'
                WHEN 'd' THEN 'SET DEFAULT'
            END AS on_update
        FROM pg_constraint c
        JOIN pg_class t  ON t.oid = c.conrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_class rt ON rt.oid = c.confrelid
        JOIN pg_attribute a  ON a.attrelid = c.conrelid  AND a.attnum = ANY(c.conkey)
        JOIN pg_attribute ra ON ra.attrelid = c.confrelid AND ra.attnum = ANY(c.confkey)
        WHERE c.contype = 'f'
          AND n.nspname = $1
          AND t.relname = $2
        ORDER BY c.conname,
                 array_position(c.conkey,  a.attnum),
                 array_position(c.confkey, ra.attnum)
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    let mut map: std::collections::BTreeMap<String, ForeignKeyInfo> =
        std::collections::BTreeMap::new();
    for r in rows {
        let name = r.constraint_name.unwrap_or_default();
        let col = r.column_name.unwrap_or_default();
        let ref_col = r.referenced_column.unwrap_or_default();
        let entry = map.entry(name.clone()).or_insert_with(|| ForeignKeyInfo {
            constraint_name: name.clone(),
            columns: vec![],
            referenced_table: r.referenced_table.unwrap_or_default(),
            referenced_columns: vec![],
            on_delete: r.on_delete.unwrap_or_else(|| "NO ACTION".to_owned()),
            on_update: r.on_update.unwrap_or_else(|| "NO ACTION".to_owned()),
        });
        entry.columns.push(col);
        entry.referenced_columns.push(ref_col);
    }
    Ok(map.into_values().collect())
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

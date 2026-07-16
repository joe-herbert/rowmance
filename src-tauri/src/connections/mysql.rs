/// MySQL/MariaDB-specific schema introspection queries.
use sqlx::MySqlPool;

use crate::connections::types::{ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo};
use crate::error::RowmanceError;

/// List all databases visible to this connection.
pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<String>, RowmanceError> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT CAST(SCHEMA_NAME AS CHAR) FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME",
    )
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
        row_count: Option<i64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            CAST(TABLE_NAME AS CHAR) AS name,
            CAST(TABLE_TYPE AS CHAR) AS table_type,
            CAST(TABLE_ROWS AS SIGNED) AS row_count
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = ?
        ORDER BY TABLE_NAME
        "#,
    )
    .bind(database)
    .fetch_all(pool)
    .await?;

    let tables: Vec<TableInfo> = rows
        .into_iter()
        .map(|r| TableInfo {
            name: r.name.unwrap_or_default(),
            table_type: if r.table_type.as_deref() == Some("VIEW") {
                "view".to_owned()
            } else {
                "table".to_owned()
            },
            row_count: r.row_count,
        })
        .collect();

    Ok(tables)
}

/// List all columns for a given table.
/// Uses EXISTS for FK detection to avoid GROUP BY on information_schema.
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
        is_foreign_key: Option<bool>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            CAST(c.COLUMN_NAME    AS CHAR) AS name,
            CAST(c.COLUMN_TYPE    AS CHAR) AS data_type,
            CAST(c.IS_NULLABLE    AS CHAR) AS nullable,
            CAST(c.COLUMN_DEFAULT AS CHAR) AS default_value,
            CAST(c.COLUMN_KEY     AS CHAR) AS column_key,
            CAST(c.EXTRA          AS CHAR) AS extra,
            CAST(c.COLUMN_COMMENT AS CHAR) AS comment,
            EXISTS (
                SELECT 1 FROM information_schema.KEY_COLUMN_USAGE kcu
                WHERE kcu.TABLE_SCHEMA           = c.TABLE_SCHEMA
                  AND kcu.TABLE_NAME             = c.TABLE_NAME
                  AND kcu.COLUMN_NAME            = c.COLUMN_NAME
                  AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
            ) AS is_foreign_key
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
                is_foreign_key: r.is_foreign_key.unwrap_or(false),
                comment: if r.comment.as_deref() == Some("") {
                    None
                } else {
                    r.comment
                },
            }
        })
        .collect())
}

/// List all columns for every table and view in the given database in one query.
/// Scans KEY_COLUMN_USAGE once into a derived table to avoid GROUP BY.
/// Returns (table_name, ColumnInfo) pairs ordered by table then ordinal position.
pub async fn list_all_columns(
    pool: &MySqlPool,
    database: &str,
) -> Result<Vec<(String, ColumnInfo)>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        table_name: Option<String>,
        name: Option<String>,
        data_type: Option<String>,
        nullable: Option<String>,
        default_value: Option<String>,
        column_key: Option<String>,
        extra: Option<String>,
        comment: Option<String>,
        is_foreign_key: Option<bool>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            CAST(c.TABLE_NAME     AS CHAR) AS table_name,
            CAST(c.COLUMN_NAME    AS CHAR) AS name,
            CAST(c.COLUMN_TYPE    AS CHAR) AS data_type,
            CAST(c.IS_NULLABLE    AS CHAR) AS nullable,
            CAST(c.COLUMN_DEFAULT AS CHAR) AS default_value,
            CAST(c.COLUMN_KEY     AS CHAR) AS column_key,
            CAST(c.EXTRA          AS CHAR) AS extra,
            CAST(c.COLUMN_COMMENT AS CHAR) AS comment,
            (fk.COLUMN_NAME IS NOT NULL)   AS is_foreign_key
        FROM information_schema.COLUMNS c
        LEFT JOIN (
            SELECT DISTINCT TABLE_NAME, COLUMN_NAME
            FROM information_schema.KEY_COLUMN_USAGE
            WHERE TABLE_SCHEMA            = ?
              AND REFERENCED_TABLE_NAME IS NOT NULL
        ) fk
          ON fk.TABLE_NAME  = c.TABLE_NAME
         AND fk.COLUMN_NAME = c.COLUMN_NAME
        WHERE c.TABLE_SCHEMA = ?
        ORDER BY c.TABLE_NAME, c.ORDINAL_POSITION
        "#,
    )
    .bind(database)
    .bind(database)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let is_pk = r.column_key.as_deref() == Some("PRI");
            let is_auto = r
                .extra
                .as_deref()
                .map(|e| e.contains("auto_increment"))
                .unwrap_or(false);
            (
                r.table_name.unwrap_or_default(),
                ColumnInfo {
                    name: r.name.unwrap_or_default(),
                    data_type: r.data_type.unwrap_or_default(),
                    nullable: r.nullable.as_deref() == Some("YES"),
                    default_value: r.default_value,
                    is_primary_key: is_pk,
                    is_auto_increment: is_auto,
                    is_foreign_key: r.is_foreign_key.unwrap_or(false),
                    comment: if r.comment.as_deref() == Some("") {
                        None
                    } else {
                        r.comment
                    },
                },
            )
        })
        .collect())
}

/// List all indexes for a given table.
pub async fn list_indexes(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        index_name: Option<String>,
        column_name: Option<String>,
        non_unique: Option<i64>,
        index_type: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            CAST(INDEX_NAME AS CHAR) AS index_name,
            CAST(COLUMN_NAME AS CHAR) AS column_name,
            NON_UNIQUE AS non_unique,
            CAST(INDEX_TYPE AS CHAR) AS index_type
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
        ORDER BY INDEX_NAME, SEQ_IN_INDEX
        "#,
    )
    .bind(database)
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
            unique: r.non_unique == Some(0),
            index_type: r.index_type.unwrap_or_else(|| "BTREE".to_owned()),
        });
        entry.columns.push(col);
    }
    Ok(map.into_values().collect())
}

/// List all foreign keys for a given table.
pub async fn list_foreign_keys(
    pool: &MySqlPool,
    database: &str,
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
            CAST(kcu.CONSTRAINT_NAME      AS CHAR) AS constraint_name,
            CAST(kcu.COLUMN_NAME          AS CHAR) AS column_name,
            CAST(kcu.REFERENCED_TABLE_NAME AS CHAR) AS referenced_table,
            CAST(kcu.REFERENCED_COLUMN_NAME AS CHAR) AS referenced_column,
            CAST(rc.DELETE_RULE           AS CHAR) AS on_delete,
            CAST(rc.UPDATE_RULE           AS CHAR) AS on_update
        FROM information_schema.KEY_COLUMN_USAGE kcu
        JOIN information_schema.REFERENTIAL_CONSTRAINTS rc
          ON rc.CONSTRAINT_SCHEMA = kcu.CONSTRAINT_SCHEMA
         AND rc.CONSTRAINT_NAME   = kcu.CONSTRAINT_NAME
        WHERE kcu.TABLE_SCHEMA = ?
          AND kcu.TABLE_NAME   = ?
          AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
        ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION
        "#,
    )
    .bind(database)
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

/// Count all rows in a table. Used by the background count task.
pub async fn count_table(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<i64, RowmanceError> {
    let db_esc = database.replace('`', "``");
    let tbl_esc = table.replace('`', "``");
    let count: i64 =
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{}`.`{}`", db_esc, tbl_esc))
            .fetch_one(pool)
            .await?;
    Ok(count)
}

/// Return the CREATE TABLE / CREATE VIEW DDL for an object.
pub async fn get_ddl(pool: &MySqlPool, database: &str, table: &str) -> Result<String, RowmanceError> {
    // SHOW CREATE TABLE works for both tables and views in MySQL/MariaDB.
    let row = sqlx::query(&format!("SHOW CREATE TABLE `{database}`.`{table}`"))
        .fetch_one(pool)
        .await?;

    // The DDL is in the second column; column name varies (Create Table / Create View).
    use sqlx::Row;
    let ddl: String = row.try_get(1).unwrap_or_default();
    Ok(ddl)
}

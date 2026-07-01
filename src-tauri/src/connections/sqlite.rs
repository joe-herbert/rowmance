/// SQLite-specific schema introspection queries.
use serde::Serialize;
use sqlx::SqlitePool;

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

/// SQLite has a single implicit schema; we return ["main"] so the connection
/// tree has a database node to expand into tables.
pub async fn list_databases(_pool: &SqlitePool) -> Result<Vec<String>, RowmanceError> {
    Ok(vec!["main".to_owned()])
}

/// List all user tables and views in the SQLite database.
pub async fn list_tables(
    pool: &SqlitePool,
    _database: &str,
) -> Result<Vec<TableInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        table_type: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT name, type AS table_type FROM sqlite_master \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    let mut tables: Vec<TableInfo> = rows
        .into_iter()
        .map(|r| TableInfo {
            name: r.name,
            table_type: if r.table_type == "view" {
                "view".to_owned()
            } else {
                "table".to_owned()
            },
            row_count: None,
        })
        .collect();

    for table in &mut tables {
        let tbl_esc = table.name.replace('"', "\"\"");
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{}\"", tbl_esc))
            .fetch_one(pool)
            .await?;
        table.row_count = Some(count);
    }

    Ok(tables)
}

/// List all columns for a given table using PRAGMA table_info.
pub async fn list_columns(
    pool: &SqlitePool,
    _database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        #[sqlx(rename = "type")]
        data_type: String,
        notnull: i64,
        dflt_value: Option<String>,
        pk: i64,
    }

    let fk_cols = foreign_key_columns(pool, table).await;

    let sql = format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\""));
    let rows = sqlx::query_as::<_, Row>(&sql).fetch_all(pool).await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let is_fk = fk_cols.contains(&r.name);
            ColumnInfo {
                name: r.name,
                data_type: r.data_type,
                nullable: r.notnull == 0,
                default_value: r.dflt_value,
                is_primary_key: r.pk != 0,
                is_auto_increment: false,
                is_foreign_key: is_fk,
                comment: None,
            }
        })
        .collect())
}

async fn foreign_key_columns(pool: &SqlitePool, table: &str) -> std::collections::HashSet<String> {
    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(rename = "from")]
        from_col: String,
    }

    let sql = format!(
        "PRAGMA foreign_key_list(\"{}\")",
        table.replace('"', "\"\"")
    );
    sqlx::query_as::<_, Row>(&sql)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.from_col)
        .collect()
}

/// List all indexes for a given table.
pub async fn list_indexes(
    pool: &SqlitePool,
    _database: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct IndexListRow {
        name: String,
        unique: i64,
    }

    #[derive(sqlx::FromRow)]
    struct IndexInfoRow {
        name: Option<String>,
    }

    let list_sql = format!("PRAGMA index_list(\"{}\")", table.replace('"', "\"\""));
    let indexes = sqlx::query_as::<_, IndexListRow>(&list_sql)
        .fetch_all(pool)
        .await?;

    let mut result = Vec::new();
    for idx in indexes {
        let info_sql = format!("PRAGMA index_info(\"{}\")", idx.name.replace('"', "\"\""));
        let cols = sqlx::query_as::<_, IndexInfoRow>(&info_sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        let columns: Vec<String> = cols.into_iter().filter_map(|r| r.name).collect();

        result.push(IndexInfo {
            name: idx.name,
            columns,
            unique: idx.unique != 0,
            index_type: "BTREE".to_owned(),
        });
    }

    Ok(result)
}

/// List all foreign keys for a given table.
pub async fn list_foreign_keys(
    pool: &SqlitePool,
    _database: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        #[sqlx(rename = "table")]
        ref_table: String,
        #[sqlx(rename = "from")]
        from_col: String,
        #[sqlx(rename = "to")]
        to_col: Option<String>,
        on_delete: String,
        on_update: String,
    }

    let sql = format!(
        "PRAGMA foreign_key_list(\"{}\")",
        table.replace('"', "\"\"")
    );
    let rows = sqlx::query_as::<_, Row>(&sql).fetch_all(pool).await?;

    let mut map: std::collections::BTreeMap<i64, ForeignKeyInfo> =
        std::collections::BTreeMap::new();
    for r in rows {
        let entry = map.entry(r.id).or_insert_with(|| ForeignKeyInfo {
            constraint_name: format!("fk_{table}_{}", r.id),
            columns: vec![],
            referenced_table: r.ref_table.clone(),
            referenced_columns: vec![],
            on_delete: r.on_delete.clone(),
            on_update: r.on_update.clone(),
        });
        entry.columns.push(r.from_col);
        if let Some(to) = r.to_col {
            entry.referenced_columns.push(to);
        }
    }

    Ok(map.into_values().collect())
}

/// Return the CREATE TABLE / CREATE VIEW DDL from sqlite_master.
pub async fn get_ddl(pool: &SqlitePool, table: &str) -> Result<String, RowmanceError> {
    let ddl: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE name = ? AND type IN ('table', 'view')",
    )
    .bind(table)
    .fetch_optional(pool)
    .await?;

    Ok(ddl.unwrap_or_default())
}

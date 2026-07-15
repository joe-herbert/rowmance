/// Tauri commands for Entity-Relationship Diagram data.
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct ErdColumn {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
}

#[derive(Debug, Serialize)]
pub struct ErdTable {
    pub name: String,
    pub columns: Vec<ErdColumn>,
}

#[derive(Debug, Serialize)]
pub struct ErdRelation {
    #[serde(rename = "fromTable")]
    pub from_table: String,
    #[serde(rename = "fromColumns")]
    pub from_columns: Vec<String>,
    #[serde(rename = "toTable")]
    pub to_table: String,
    #[serde(rename = "toColumns")]
    pub to_columns: Vec<String>,
    #[serde(rename = "constraintName")]
    pub constraint_name: String,
}

#[derive(Debug, Serialize)]
pub struct ErdGraph {
    pub nodes: Vec<ErdTable>,
    pub edges: Vec<ErdRelation>,
}

#[tauri::command]
pub async fn erd_get_graph(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
) -> Result<ErdGraph, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool, _) => get_graph_mysql(pool, &database).await,
        RemotePool::Postgres(pool) => get_graph_postgres(pool, &database).await,
        RemotePool::Sqlite(pool) => get_graph_sqlite(pool).await,
        RemotePool::SqlServer(pool, _) => get_graph_sqlserver(pool, &database).await,
    }
}

async fn get_graph_mysql(pool: &sqlx::MySqlPool, database: &str) -> Result<ErdGraph, AppError> {
    #[derive(sqlx::FromRow)]
    struct ColRow {
        table_name: Option<String>,
        column_name: Option<String>,
        data_type: Option<String>,
        column_key: Option<String>,
    }

    let col_rows = sqlx::query_as::<_, ColRow>(
        r#"
        SELECT
            CAST(TABLE_NAME  AS CHAR) AS table_name,
            CAST(COLUMN_NAME AS CHAR) AS column_name,
            CAST(COLUMN_TYPE AS CHAR) AS data_type,
            CAST(COLUMN_KEY  AS CHAR) AS column_key
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = ?
        ORDER BY TABLE_NAME, ORDINAL_POSITION
        "#,
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let nodes = group_into_tables(col_rows.into_iter().map(|r| {
        (
            r.table_name.unwrap_or_default(),
            ErdColumn {
                name: r.column_name.unwrap_or_default(),
                data_type: r.data_type.unwrap_or_default(),
                is_primary_key: r.column_key.as_deref() == Some("PRI"),
            },
        )
    }));

    #[derive(sqlx::FromRow)]
    struct FkRow {
        constraint_name: Option<String>,
        table_name: Option<String>,
        column_name: Option<String>,
        referenced_table_name: Option<String>,
        referenced_column_name: Option<String>,
    }

    let fk_rows = sqlx::query_as::<_, FkRow>(
        r#"
        SELECT
            CAST(kcu.CONSTRAINT_NAME         AS CHAR) AS constraint_name,
            CAST(kcu.TABLE_NAME              AS CHAR) AS table_name,
            CAST(kcu.COLUMN_NAME             AS CHAR) AS column_name,
            CAST(kcu.REFERENCED_TABLE_NAME   AS CHAR) AS referenced_table_name,
            CAST(kcu.REFERENCED_COLUMN_NAME  AS CHAR) AS referenced_column_name
        FROM information_schema.KEY_COLUMN_USAGE kcu
        JOIN information_schema.TABLE_CONSTRAINTS tc
          ON tc.CONSTRAINT_NAME  = kcu.CONSTRAINT_NAME
         AND tc.TABLE_SCHEMA     = kcu.TABLE_SCHEMA
         AND tc.TABLE_NAME       = kcu.TABLE_NAME
        WHERE kcu.TABLE_SCHEMA = ?
          AND tc.CONSTRAINT_TYPE = 'FOREIGN KEY'
          AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
        ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION
        "#,
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let edges = build_edges_from_fk_rows(fk_rows.into_iter().map(|r| FkNorm {
        constraint_name: r.constraint_name.unwrap_or_default(),
        table_name: r.table_name.unwrap_or_default(),
        column_name: r.column_name.unwrap_or_default(),
        referenced_table_name: r.referenced_table_name.unwrap_or_default(),
        referenced_column_name: r.referenced_column_name.unwrap_or_default(),
    }));

    Ok(ErdGraph { nodes, edges })
}

async fn get_graph_postgres(pool: &sqlx::PgPool, schema: &str) -> Result<ErdGraph, AppError> {
    #[derive(sqlx::FromRow)]
    struct ColRow {
        table_name: Option<String>,
        column_name: Option<String>,
        data_type: Option<String>,
        is_primary_key: Option<bool>,
    }

    let col_rows = sqlx::query_as::<_, ColRow>(
        r#"
        SELECT
            c.table_name,
            c.column_name,
            c.udt_name AS data_type,
            EXISTS (
                SELECT 1
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                  ON kcu.constraint_name = tc.constraint_name
                 AND kcu.table_schema    = tc.table_schema
                WHERE tc.constraint_type = 'PRIMARY KEY'
                  AND kcu.table_schema   = $1
                  AND kcu.table_name     = c.table_name
                  AND kcu.column_name    = c.column_name
            ) AS is_primary_key
        FROM information_schema.columns c
        WHERE c.table_schema = $1
        ORDER BY c.table_name, c.ordinal_position
        "#,
    )
    .bind(schema)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let nodes = group_into_tables(col_rows.into_iter().map(|r| {
        (
            r.table_name.unwrap_or_default(),
            ErdColumn {
                name: r.column_name.unwrap_or_default(),
                data_type: r.data_type.unwrap_or_default(),
                is_primary_key: r.is_primary_key.unwrap_or(false),
            },
        )
    }));

    #[derive(sqlx::FromRow)]
    struct FkRow {
        constraint_name: Option<String>,
        table_name: Option<String>,
        column_name: Option<String>,
        referenced_table_name: Option<String>,
        referenced_column_name: Option<String>,
    }

    let fk_rows = sqlx::query_as::<_, FkRow>(
        r#"
        SELECT
            tc.constraint_name,
            kcu.table_name,
            kcu.column_name,
            ccu.table_name  AS referenced_table_name,
            ccu.column_name AS referenced_column_name
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
          ON kcu.constraint_name = tc.constraint_name
         AND kcu.table_schema    = tc.table_schema
        JOIN information_schema.constraint_column_usage ccu
          ON ccu.constraint_name = tc.constraint_name
         AND ccu.table_schema    = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
          AND tc.table_schema    = $1
        ORDER BY tc.constraint_name, kcu.ordinal_position
        "#,
    )
    .bind(schema)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let edges = build_edges_from_fk_rows(fk_rows.into_iter().map(|r| FkNorm {
        constraint_name: r.constraint_name.unwrap_or_default(),
        table_name: r.table_name.unwrap_or_default(),
        column_name: r.column_name.unwrap_or_default(),
        referenced_table_name: r.referenced_table_name.unwrap_or_default(),
        referenced_column_name: r.referenced_column_name.unwrap_or_default(),
    }));

    Ok(ErdGraph { nodes, edges })
}

async fn get_graph_sqlite(pool: &sqlx::SqlitePool) -> Result<ErdGraph, AppError> {
    #[derive(sqlx::FromRow)]
    struct TableRow {
        name: String,
    }

    let tables = sqlx::query_as::<_, TableRow>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let mut nodes: Vec<ErdTable> = Vec::new();
    let mut all_fk_edges: Vec<ErdRelation> = Vec::new();

    for table in &tables {
        #[derive(sqlx::FromRow)]
        struct ColRow {
            name: String,
            #[sqlx(rename = "type")]
            data_type: String,
            pk: i64,
        }

        let col_sql = format!("PRAGMA table_info(\"{}\")", table.name.replace('"', "\"\""));
        let cols = sqlx::query_as::<_, ColRow>(&col_sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        let columns: Vec<ErdColumn> = cols
            .into_iter()
            .map(|c| ErdColumn {
                name: c.name,
                data_type: c.data_type,
                is_primary_key: c.pk != 0,
            })
            .collect();

        nodes.push(ErdTable {
            name: table.name.clone(),
            columns,
        });

        #[derive(sqlx::FromRow)]
        struct FkRow {
            id: i64,
            #[sqlx(rename = "table")]
            ref_table: String,
            #[sqlx(rename = "from")]
            from_col: String,
            #[sqlx(rename = "to")]
            to_col: Option<String>,
        }

        let fk_sql = format!(
            "PRAGMA foreign_key_list(\"{}\")",
            table.name.replace('"', "\"\"")
        );
        let fk_rows = sqlx::query_as::<_, FkRow>(&fk_sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        let mut fk_map: std::collections::BTreeMap<i64, (String, Vec<String>, Vec<String>)> =
            std::collections::BTreeMap::new();
        for r in fk_rows {
            let entry = fk_map
                .entry(r.id)
                .or_insert_with(|| (r.ref_table.clone(), vec![], vec![]));
            entry.1.push(r.from_col);
            if let Some(to) = r.to_col {
                entry.2.push(to);
            }
        }
        for (id, (ref_table, from_cols, to_cols)) in fk_map {
            all_fk_edges.push(ErdRelation {
                from_table: table.name.clone(),
                from_columns: from_cols,
                to_table: ref_table,
                to_columns: to_cols,
                constraint_name: format!("fk_{}_{}", table.name, id),
            });
        }
    }

    Ok(ErdGraph {
        nodes,
        edges: all_fk_edges,
    })
}

/// Fold an ordered iterator of (table_name, column) pairs into ErdTable nodes,
/// preserving the original ordering from the database query.
fn group_into_tables(rows: impl Iterator<Item = (String, ErdColumn)>) -> Vec<ErdTable> {
    let mut nodes: Vec<ErdTable> = Vec::new();
    for (table_name, col) in rows {
        if let Some(last) = nodes.last_mut() {
            if last.name == table_name {
                last.columns.push(col);
                continue;
            }
        }
        nodes.push(ErdTable {
            name: table_name,
            columns: vec![col],
        });
    }
    nodes
}

struct FkNorm {
    constraint_name: String,
    table_name: String,
    column_name: String,
    referenced_table_name: String,
    referenced_column_name: String,
}

fn build_edges_from_fk_rows(rows: impl Iterator<Item = FkNorm>) -> Vec<ErdRelation> {
    // Accumulate columns per constraint in query order (rows already sorted by constraint_name, ordinal).
    let mut edges: Vec<ErdRelation> = Vec::new();
    for row in rows {
        if let Some(last) = edges.last_mut() {
            if last.constraint_name == row.constraint_name {
                last.from_columns.push(row.column_name);
                last.to_columns.push(row.referenced_column_name);
                continue;
            }
        }
        edges.push(ErdRelation {
            from_table: row.table_name,
            from_columns: vec![row.column_name],
            to_table: row.referenced_table_name,
            to_columns: vec![row.referenced_column_name],
            constraint_name: row.constraint_name,
        });
    }
    edges
}

async fn get_graph_sqlserver(
    pool: &bb8::Pool<bb8_tiberius::ConnectionManager>,
    schema: &str,
) -> Result<ErdGraph, AppError> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;

    let all_cols = crate::connections::sqlserver::list_all_columns(&mut *conn, schema)
        .await
        .map_err(AppError::from)?;

    let col_pairs: Vec<(String, ErdColumn)> = all_cols
        .into_iter()
        .map(|(table_name, c)| {
            (
                table_name,
                ErdColumn {
                    name: c.name,
                    data_type: c.data_type,
                    is_primary_key: c.is_primary_key,
                },
            )
        })
        .collect();
    let nodes = group_into_tables(col_pairs.into_iter());

    let mut all_fk_edges: Vec<ErdRelation> = Vec::new();
    for table in &nodes {
        let fks = crate::connections::sqlserver::list_foreign_keys(&mut *conn, schema, &table.name)
            .await
            .map_err(AppError::from)?;
        for fk in fks {
            all_fk_edges.push(ErdRelation {
                from_table: table.name.clone(),
                from_columns: fk.columns,
                to_table: fk.referenced_table,
                to_columns: fk.referenced_columns,
                constraint_name: fk.constraint_name,
            });
        }
    }

    Ok(ErdGraph {
        nodes,
        edges: all_fk_edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, pk: bool) -> ErdColumn {
        ErdColumn {
            name: name.to_owned(),
            data_type: "int".to_owned(),
            is_primary_key: pk,
        }
    }

    fn fk(
        constraint: &str,
        from_table: &str,
        from_col: &str,
        to_table: &str,
        to_col: &str,
    ) -> FkNorm {
        FkNorm {
            constraint_name: constraint.to_owned(),
            table_name: from_table.to_owned(),
            column_name: from_col.to_owned(),
            referenced_table_name: to_table.to_owned(),
            referenced_column_name: to_col.to_owned(),
        }
    }

    #[test]
    fn group_into_tables_empty() {
        let nodes = group_into_tables(std::iter::empty());
        assert!(nodes.is_empty());
    }

    #[test]
    fn group_into_tables_single_table_multiple_columns() {
        let rows = vec![
            ("users".to_owned(), col("id", true)),
            ("users".to_owned(), col("name", false)),
        ];
        let nodes = group_into_tables(rows.into_iter());
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "users");
        assert_eq!(nodes[0].columns.len(), 2);
        assert!(nodes[0].columns[0].is_primary_key);
    }

    #[test]
    fn group_into_tables_multiple_tables() {
        let rows = vec![
            ("orders".to_owned(), col("id", true)),
            ("users".to_owned(), col("id", true)),
            ("users".to_owned(), col("email", false)),
        ];
        let nodes = group_into_tables(rows.into_iter());
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "orders");
        assert_eq!(nodes[1].name, "users");
        assert_eq!(nodes[1].columns.len(), 2);
    }

    #[test]
    fn build_edges_empty() {
        let edges = build_edges_from_fk_rows(std::iter::empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn build_edges_single_column_fk() {
        let rows = vec![fk("fk_a", "orders", "user_id", "users", "id")];
        let edges = build_edges_from_fk_rows(rows.into_iter());
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].constraint_name, "fk_a");
        assert_eq!(edges[0].from_table, "orders");
        assert_eq!(edges[0].from_columns, vec!["user_id"]);
        assert_eq!(edges[0].to_table, "users");
        assert_eq!(edges[0].to_columns, vec!["id"]);
    }

    #[test]
    fn build_edges_composite_fk_merged() {
        let rows = vec![
            fk("fk_comp", "items", "order_id", "orders", "id"),
            fk("fk_comp", "items", "product_id", "orders", "product_id"),
        ];
        let edges = build_edges_from_fk_rows(rows.into_iter());
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_columns, vec!["order_id", "product_id"]);
        assert_eq!(edges[0].to_columns, vec!["id", "product_id"]);
    }

    #[test]
    fn build_edges_distinct_constraints_not_merged() {
        let rows = vec![
            fk("fk_a", "child", "a_id", "parent_a", "id"),
            fk("fk_b", "child", "b_id", "parent_b", "id"),
        ];
        let edges = build_edges_from_fk_rows(rows.into_iter());
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].constraint_name, "fk_a");
        assert_eq!(edges[1].constraint_name, "fk_b");
    }
}

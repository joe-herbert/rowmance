/// SQLite engine implementation of the `DatabaseEngine` trait.
use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::Column;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;
use sqlx::TypeInfo;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use crate::connections::engine::{DatabaseEngine, EngineTransaction};
use crate::connections::types::{
    BulkColumnRow, ColumnInfo, ColumnMeta, ErdColumn, ErdGraph, ErdRelation, ErdTable,
    EngineQueryResult, ExplainResult, ForeignKeyInfo, IndexInfo, RowChange, RowDelete, TableInfo,
};
use crate::error::RowmanceError;

pub struct SqliteEngine {
    pub pool: sqlx::SqlitePool,
}

#[async_trait]
impl DatabaseEngine for SqliteEngine {
    fn quote(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn placeholder(&self, _n: usize) -> String {
        "?".to_string()
    }

    fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String {
        if limit == 0 || sql_has_top_level_limit(sql) {
            sql.to_string()
        } else {
            format!("{sql} LIMIT {limit} OFFSET {offset}")
        }
    }

    fn parse_returns_rows(&self, sql: &str) -> bool {
        match Parser::parse_sql(&SQLiteDialect {}, sql) {
            Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
            Err(_) => returns_rows_heuristic(sql),
        }
    }

    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> {
        crate::connections::sqlite::list_databases(&self.pool).await
    }

    async fn list_tables(&self, database: &str, _instance_db: Option<&str>) -> Result<Vec<TableInfo>, RowmanceError> {
        crate::connections::sqlite::list_tables(&self.pool, database).await
    }

    async fn list_columns(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<ColumnInfo>, RowmanceError> {
        crate::connections::sqlite::list_columns(&self.pool, database, table).await
    }

    async fn list_all_columns(&self, database: &str, _instance_db: Option<&str>) -> Result<Vec<BulkColumnRow>, RowmanceError> {
        let pairs = crate::connections::sqlite::list_all_columns(&self.pool, database).await?;
        Ok(pairs
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
            .collect())
    }

    async fn list_indexes(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<IndexInfo>, RowmanceError> {
        crate::connections::sqlite::list_indexes(&self.pool, database, table).await
    }

    async fn list_foreign_keys(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
        crate::connections::sqlite::list_foreign_keys(&self.pool, database, table).await
    }

    async fn count_table(&self, _database: &str, table: &str, _instance_db: Option<&str>) -> Result<i64, RowmanceError> {
        crate::connections::sqlite::count_table(&self.pool, table).await
    }

    async fn get_ddl(&self, _database: &str, table: &str, _instance_db: Option<&str>) -> Result<String, RowmanceError> {
        crate::connections::sqlite::get_ddl(&self.pool, table).await
    }

    async fn execute(
        &self,
        sql: &str,
        _database: Option<&str>,
        _instance_db: Option<&str>,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        execute_on_sqlite_conn(&mut conn, sql, page_size, offset).await
    }

    async fn execute_ddl(&self, sql: &str) -> Result<(), RowmanceError> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn count_query_rows(&self, sql: &str, _database: Option<&str>, _instance_db: Option<&str>) -> Option<i64> {
        let sql_trimmed = sql.trim_end_matches(';');
        let count_sql = format!("SELECT COUNT(*) FROM ({sql_trimmed}) AS _count_query");
        let mut conn = self.pool.acquire().await.ok()?;
        sqlx::query_scalar::<_, i64>(&count_sql)
            .fetch_one(&mut *conn)
            .await
            .ok()
    }

    async fn apply_changes(
        &self,
        _database: &str,
        table: &str,
        _instance_db: Option<&str>,
        updates: &[RowChange],
        inserts: &[HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        sqlx::query("BEGIN")
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;

        let result = apply_all_sqlite(&mut conn, table, updates, inserts, deletes).await;

        match result {
            Ok(counts) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(RowmanceError::Database)?;
                Ok(counts)
            }
            Err(e) => {
                sqlx::query("ROLLBACK").execute(&mut *conn).await.ok();
                Err(e)
            }
        }
    }

    async fn ping(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn begin_transaction(
        &self,
        _database: Option<&str>,
    ) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        sqlx::query("BEGIN")
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;
        Ok(Box::new(SqliteTransaction { conn }))
    }

    async fn explain(&self, sql: &str, _database: Option<&str>, _instance_db: Option<&str>) -> Result<ExplainResult, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let rows = sqlx::query(&explain_sql)
            .fetch_all(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;
        let plans: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                let id: i64 = row.try_get("id").unwrap_or(0);
                let parent: i64 = row.try_get("parent").unwrap_or(0);
                let detail: String = row.try_get("detail").unwrap_or_default();
                serde_json::json!({ "id": id, "parent": parent, "detail": detail })
            })
            .collect();
        Ok(ExplainResult {
            raw_json: serde_json::to_string(&plans)
                .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?,
            dialect: "sqlite_queryplan".to_string(),
        })
    }

    async fn begin_session(&self) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        Ok(Box::new(SqliteTransaction { conn }))
    }

    async fn get_erd_graph(&self, _database: &str, _instance_db: Option<&str>) -> Result<ErdGraph, RowmanceError> {
        #[derive(sqlx::FromRow)]
        struct TableRow {
            name: String,
        }

        let tables = sqlx::query_as::<_, TableRow>(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

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
                .fetch_all(&self.pool)
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
                .fetch_all(&self.pool)
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

    async fn import_csv(
        &self,
        _database: &str,
        table: &str,
        _instance_db: Option<&str>,
        headers: &[String],
        rows: &[Vec<String>],
        create_table: bool,
    ) -> Result<u64, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        if create_table {
            let col_defs: Vec<String> = headers
                .iter()
                .map(|h| format!("\"{}\" TEXT", h.replace('"', "\"\"")))
                .collect();
            let ddl = format!(
                "CREATE TABLE IF NOT EXISTS \"{}\" ({})",
                table.replace('"', "\"\""),
                col_defs.join(", ")
            );
            sqlx::query(&ddl)
                .execute(&mut *conn)
                .await
                .map_err(RowmanceError::Database)?;
        }

        let col_names: Vec<String> = headers
            .iter()
            .map(|h| format!("\"{}\"", h.replace('"', "\"\"")))
            .collect();
        let placeholders: Vec<&str> = headers.iter().map(|_| "?").collect();
        let sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({})",
            table.replace('"', "\"\""),
            col_names.join(", "),
            placeholders.join(", ")
        );

        let mut inserted = 0u64;
        for row in rows {
            let mut q = sqlx::query(&sql);
            for field in row {
                q = q.bind(field.as_str());
            }
            q.execute(&mut *conn)
                .await
                .map_err(RowmanceError::Database)?;
            inserted += 1;
        }

        Ok(inserted)
    }
}

// ── SqliteTransaction ─────────────────────────────────────────────────────────

pub struct SqliteTransaction {
    conn: sqlx::pool::PoolConnection<sqlx::Sqlite>,
}

#[async_trait]
impl EngineTransaction for SqliteTransaction {
    async fn execute(
        &mut self,
        sql: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        execute_on_sqlite_conn(&mut self.conn, sql, page_size, offset).await
    }

    async fn apply_changes(
        &mut self,
        _database: &str,
        table: &str,
        _instance_db: Option<&str>,
        updates: &[RowChange],
        inserts: &[std::collections::HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        apply_all_sqlite(&mut self.conn, table, updates, inserts, deletes).await
    }

    async fn execute_raw(&mut self, sql: &str) -> Result<(), RowmanceError> {
        sqlx::query(sql)
            .execute(&mut *self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn commit(&mut self) -> Result<(), RowmanceError> {
        sqlx::query("COMMIT")
            .execute(&mut *self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn rollback(&mut self) -> Result<(), RowmanceError> {
        sqlx::query("ROLLBACK")
            .execute(&mut *self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn switch_context(&mut self, _database: &str) -> Result<(), RowmanceError> {
        // SQLite has no schema-switching concept
        Ok(())
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

async fn execute_on_sqlite_conn(
    conn: &mut sqlx::sqlite::SqliteConnection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> Result<EngineQueryResult, RowmanceError> {
    let exec_sql = sql.trim_end_matches(';');

    if returns_rows_heuristic_sqlite(exec_sql) {
        let limit = page_size as u64;
        let off = offset as u64;
        let paginated = if limit == 0 || sql_has_top_level_limit(exec_sql) {
            exec_sql.to_string()
        } else {
            format!("{exec_sql} LIMIT {limit} OFFSET {off}")
        };
        let rows = sqlx::query(&paginated)
            .fetch_all(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;

        let columns: Vec<ColumnMeta> = if let Some(first) = rows.first() {
            first
                .columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_owned(),
                    data_type: c.type_info().name().to_lowercase(),
                    nullable: true,
                    is_primary_key: false,
                    is_foreign_key: false,
                })
                .collect()
        } else {
            match conn.prepare(&paginated).await {
                Ok(stmt) => stmt
                    .columns()
                    .iter()
                    .map(|c| ColumnMeta {
                        name: c.name().to_owned(),
                        data_type: c.type_info().name().to_lowercase(),
                        nullable: true,
                        is_primary_key: false,
                        is_foreign_key: false,
                    })
                    .collect(),
                Err(_) => vec![],
            }
        };

        let data: Vec<Vec<serde_json::Value>> = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| sqlite_value_to_json(row, i))
                    .collect()
            })
            .collect();

        Ok(EngineQueryResult {
            columns,
            rows: data,
            affected_rows: None,
        })
    } else {
        let result = sqlx::query(exec_sql)
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;
        Ok(EngineQueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: Some(result.rows_affected()),
        })
    }
}

fn returns_rows_heuristic_sqlite(sql: &str) -> bool {
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser;
    match Parser::parse_sql(&SQLiteDialect {}, sql) {
        Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
        Err(_) => returns_rows_heuristic(sql),
    }
}

async fn apply_all_sqlite(
    conn: &mut sqlx::sqlite::SqliteConnection,
    table: &str,
    updates: &[RowChange],
    inserts: &[HashMap<String, serde_json::Value>],
    deletes: &[RowDelete],
) -> Result<(u64, u64, u64), RowmanceError> {
    let q = |ident: &str| format!("\"{}\"", ident.replace('"', "\"\""));

    let mut updated = 0u64;
    let mut inserted = 0u64;
    let mut deleted = 0u64;

    // Updates
    for change in updates {
        if change.changes.is_empty() || change.primary_keys.is_empty() {
            continue;
        }
        let set_clause: Vec<String> = change
            .changes
            .keys()
            .map(|col| format!("{} = ?", q(col)))
            .collect();
        let where_pairs: Vec<(&String, &serde_json::Value)> = change.primary_keys.iter().collect();
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_bind: Vec<&serde_json::Value> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = ?", q(col)));
                where_bind.push(val);
            }
        }
        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            q(table),
            set_clause.join(", "),
            where_parts.join(" AND ")
        );
        let mut query = sqlx::query(&sql);
        for val in change.changes.values() {
            query = bind_sqlite_value(query, val);
        }
        for val in &where_bind {
            query = bind_sqlite_value(query, val);
        }
        updated += query
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?
            .rows_affected();
    }

    // Inserts
    for values in inserts {
        if values.is_empty() {
            continue;
        }
        let cols: Vec<(&String, &serde_json::Value)> = values.iter().collect();
        let col_list: Vec<String> = cols.iter().map(|(c, _)| q(c)).collect();
        let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            q(table),
            col_list.join(", "),
            placeholders.join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (_, val) in &cols {
            query = bind_sqlite_value(query, val);
        }
        query
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;
        inserted += 1;
    }

    // Deletes
    for row_del in deletes {
        if row_del.primary_keys.is_empty() {
            continue;
        }
        let where_pairs: Vec<(&String, &serde_json::Value)> = row_del.primary_keys.iter().collect();
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_bind: Vec<&serde_json::Value> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = ?", q(col)));
                where_bind.push(val);
            }
        }
        let sql = format!(
            "DELETE FROM {} WHERE {}",
            q(table),
            where_parts.join(" AND ")
        );
        let mut query = sqlx::query(&sql);
        for val in &where_bind {
            query = bind_sqlite_value(query, val);
        }
        deleted += query
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?
            .rows_affected();
    }

    Ok((updated, inserted, deleted))
}

fn sqlite_value_to_json(row: &sqlx::sqlite::SqliteRow, idx: usize) -> serde_json::Value {
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return v
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return v
            .and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<u64>, _>(idx) {
        return v
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
        return v
            .map(serde_json::Value::Bool)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return v
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
    }
    serde_json::Value::Null
}

fn bind_sqlite_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    value: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match value {
        serde_json::Value::String(s) => query.bind(s.as_str()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => query.bind(*b),
        serde_json::Value::Null => query.bind(Option::<String>::None),
        other => query.bind(other.to_string()),
    }
}

fn sql_has_top_level_limit(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut depth = 0i32;

    while i < len {
        match bytes[i] {
            q @ (b'\'' | b'"' | b'`') => {
                i += 1;
                while i < len {
                    if bytes[i] == q {
                        i += 1;
                        if i < len && bytes[i] == q {
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => {
                if depth == 0 && i + 5 <= len {
                    let word = &sql[i..i + 5];
                    if word.eq_ignore_ascii_case("limit") {
                        let before_ok = i == 0
                            || (!bytes[i - 1].is_ascii_alphabetic() && bytes[i - 1] != b'_');
                        let after_ok = bytes
                            .get(i + 5)
                            .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');
                        if before_ok && after_ok {
                            return true;
                        }
                    }
                }
                i += 1;
            }
        }
    }
    false
}

fn stmt_returns_rows(stmt: &sqlparser::ast::Statement) -> bool {
    use sqlparser::ast::Statement;
    match stmt {
        Statement::Query(_) => true,
        Statement::Insert(i) => i.returning.is_some(),
        Statement::Update(u) => u.returning.is_some(),
        Statement::Delete(d) => d.returning.is_some(),
        Statement::ShowVariable { .. }
        | Statement::ShowColumns { .. }
        | Statement::ShowCreate { .. }
        | Statement::ShowTables { .. }
        | Statement::ShowStatus { .. }
        | Statement::ShowDatabases { .. }
        | Statement::ShowFunctions { .. } => true,
        Statement::Pragma { value, .. } => value.is_some(),
        _ => false,
    }
}

fn returns_rows_heuristic(sql: &str) -> bool {
    let mut s = sql.trim();
    loop {
        s = s.trim_start();
        if s.starts_with("--") {
            s = s.split_once('\n').map(|x| x.1).unwrap_or("");
        } else if s.starts_with("/*") {
            s = s.find("*/").map(|i| &s[i + 2..]).unwrap_or("");
        } else {
            break;
        }
    }
    let upper = s.trim().to_uppercase();
    upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("TABLE ")
        || upper.starts_with("VALUES")
        || upper.starts_with("SHOW")
        || upper.starts_with("PRAGMA")
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("DESC ")
}

// ── Dialect info ──────────────────────────────────────────────────────────────

pub fn dialect_info(db_type: &str) -> Option<crate::connections::types::DialectInfo> {
    use crate::connections::types::DialectInfo;
    match db_type {
        "sqlite" => Some(DialectInfo {
            identifier_open: "\"".into(),
            identifier_close: "\"".into(),
            identifier_escape: "\"\"".into(),
            uses_schema: false,
            db_label: "Database".into(),
            has_instance_databases: false,
            select_top: false,
            boolean_literals: false,
            uses_ilike: false,
            cast_to_text: "CAST({col} AS TEXT)".into(),
            supports_user_management: false,
            host_based_users: false,
            supports_roles: false,
            detects_sql_variables: false,
            warns_tx_database_mismatch: false,
            display_name: "SQLite".into(),
            default_column_type: "INTEGER".into(),
            common_column_types: vec![
                "INTEGER", "TEXT", "REAL", "BLOB", "NUMERIC",
            ].into_iter().map(String::from).collect(),
            supports_auto_increment: false,
            supports_column_comment: false,
            supports_change_column: false,
            supports_rename_column: true,
            uses_foreign_key_keyword: false,
            drop_index_syntax: "simple".into(),
            default_new_column_type: "TEXT".into(),
            uses_database_keyword: false,
            drop_schema_cascade: false,
            is_file_based: true,
            default_port: 0,
            url_schemes: vec!["sqlite".into()],
            url_template: "sqlite://{host}".into(),
            fk_violation: None,
            editor_dialect: "sql".into(),
            explain_format: "sqlite_queryplan".into(),
            system_databases: vec![],
            file_extensions: vec!["sqlite".into(), "db".into(), "sqlite3".into()],
        }),
        _ => None,
    }
}

// ── Pool adapter ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SqlitePoolAdapter {
    pub pool: sqlx::SqlitePool,
}

#[async_trait]
impl crate::connections::engine::PoolAdapter for SqlitePoolAdapter {
    async fn disconnect(&self) { self.pool.close().await; }
    async fn ping(&self) -> bool { true } // local file, always alive
    fn get_engine(&self) -> std::sync::Arc<dyn crate::connections::engine::DatabaseEngine> {
        std::sync::Arc::new(SqliteEngine { pool: self.pool.clone() })
    }
}

// ── Pool creation ─────────────────────────────────────────────────────────────

pub async fn create_pool(
    host: &str,
    pool_max: u32,
    read_only: bool,
) -> Result<Box<dyn crate::connections::engine::PoolAdapter>, crate::error::RowmanceError> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    // For SQLite, `host` holds the file path (or `:memory:`).
    let opts = SqliteConnectOptions::new()
        .filename(host)
        .read_only(read_only)
        .create_if_missing(!read_only);
    let p = SqlitePoolOptions::new()
        .min_connections(0)
        .max_connections(pool_max)
        .connect_with(opts)
        .await?;
    Ok(Box::new(SqlitePoolAdapter { pool: p }))
}

/// SQL Server engine implementation of the `DatabaseEngine` trait.
use std::collections::HashMap;

use async_trait::async_trait;
use futures::TryStreamExt;
use sqlparser::dialect::MsSqlDialect;
use sqlparser::parser::Parser;

use crate::connections::engine::{DatabaseEngine, EngineTransaction};
use crate::connections::erd::group_into_tables;
use crate::connections::types::{
    BulkColumnRow, ColumnInfo, ColumnMeta, ErdColumn, ErdGraph, ErdRelation, EngineQueryResult,
    ExplainResult, ForeignKeyInfo, IndexInfo, RowChange, RowDelete, TableInfo,
};
use crate::error::RowmanceError;

type MssqlPool = bb8::Pool<bb8_tiberius::ConnectionManager>;
type MssqlConn = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

pub struct SqlServerEngine {
    pub pool: MssqlPool,
    pub read_only: bool,
}

#[async_trait]
impl DatabaseEngine for SqlServerEngine {
    fn quote(&self, ident: &str) -> String {
        format!("[{}]", ident.replace(']', "]]"))
    }

    fn placeholder(&self, n: usize) -> String {
        format!("@P{n}")
    }

    fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String {
        if limit == 0 {
            return sql.to_string();
        }
        let upper = sql.to_uppercase();
        if upper.contains("FETCH NEXT") || upper.contains("FETCH FIRST") {
            return sql.to_string();
        }
        let trimmed = upper.trim_start();
        if trimmed.starts_with("SELECT TOP") || trimmed.starts_with("WITH ") {
            return sql.to_string();
        }
        if sql_has_top_level_order_by(sql) {
            format!("{sql} OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
        } else {
            format!("{sql} ORDER BY (SELECT NULL) OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
        }
    }

    fn parse_returns_rows(&self, sql: &str) -> bool {
        match Parser::parse_sql(&MsSqlDialect {}, sql) {
            Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
            Err(_) => returns_rows_heuristic(sql),
        }
    }

    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::list_databases(&mut conn).await
    }

    async fn list_tables(&self, database: &str) -> Result<Vec<TableInfo>, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::list_tables(&mut conn, database).await
    }

    async fn list_columns(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::list_columns(&mut conn, database, table).await
    }

    async fn list_all_columns(&self, database: &str) -> Result<Vec<BulkColumnRow>, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let pairs = crate::connections::sqlserver::list_all_columns(&mut conn, database).await?;
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
    ) -> Result<Vec<IndexInfo>, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::list_indexes(&mut conn, database, table).await
    }

    async fn list_foreign_keys(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::list_foreign_keys(&mut conn, database, table).await
    }

    async fn count_table(&self, database: &str, table: &str) -> Result<i64, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::count_table(&mut conn, database, table).await
    }

    async fn get_ddl(&self, database: &str, table: &str) -> Result<String, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::get_ddl(&mut conn, database, table).await
    }

    async fn execute(
        &self,
        sql: &str,
        _database: Option<&str>,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;

        execute_on_sqlserver_conn(&mut conn, sql, page_size, offset).await
    }

    async fn execute_ddl(&self, sql: &str) -> Result<(), RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::exec_simple(&mut conn, sql).await
    }

    async fn count_query_rows(&self, sql: &str, _database: Option<&str>) -> Option<i64> {
        let sql_trimmed = sql.trim_end_matches(';');
        let count_sql = format!("SELECT COUNT(*) FROM ({sql_trimmed}) AS _count_query");
        let mut conn = self.pool.get().await.ok()?;
        let stream = conn.simple_query(&count_sql).await.ok()?;
        let rows = stream.into_first_result().await.ok()?;
        if let Some(row) = rows.first() {
            row.try_get::<i32, _>(0)
                .ok()
                .flatten()
                .map(|n| n as i64)
                .or_else(|| row.try_get::<i64, _>(0).ok().flatten())
        } else {
            None
        }
    }

    async fn apply_changes(
        &self,
        database: &str,
        table: &str,
        updates: &[RowChange],
        inserts: &[HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;

        crate::connections::sqlserver::exec_simple(&mut conn, "BEGIN TRANSACTION").await?;

        let result =
            apply_all_sqlserver(&mut conn, database, table, updates, inserts, deletes).await;

        match result {
            Ok(counts) => {
                crate::connections::sqlserver::exec_simple(&mut conn, "COMMIT TRANSACTION")
                    .await?;
                Ok(counts)
            }
            Err(e) => {
                crate::connections::sqlserver::exec_simple(&mut conn, "ROLLBACK TRANSACTION")
                    .await
                    .ok();
                Err(e)
            }
        }
    }

    async fn ping(&self) -> bool {
        match self.pool.get().await {
            Ok(mut conn) => conn.simple_query("SELECT 1").await.is_ok(),
            Err(_) => false,
        }
    }

    async fn begin_transaction(
        &self,
        _database: Option<&str>,
    ) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let mut conn = self
            .pool
            .get_owned()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        crate::connections::sqlserver::exec_simple(&mut conn, "BEGIN TRANSACTION").await?;
        Ok(Box::new(SqlServerTransaction { conn }))
    }

    async fn explain(&self, sql: &str, _database: Option<&str>) -> Result<ExplainResult, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let explain_sql = format!("SET SHOWPLAN_XML ON; {sql}; SET SHOWPLAN_XML OFF");
        let mut stream = conn
            .simple_query(explain_sql.as_str())
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let mut plan_xml = String::new();
        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?
        {
            if let tiberius::QueryItem::Row(row) = item {
                if let Some(tiberius::ColumnData::String(Some(s))) = row.into_iter().next() {
                    plan_xml = s.into_owned();
                    break;
                }
            }
        }
        let plans = serde_json::json!([{ "xml": plan_xml }]);
        Ok(ExplainResult { raw_json: plans.to_string(), dialect: "sqlserver_xml".to_string() })
    }

    async fn begin_session(&self) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let conn = self
            .pool
            .get_owned()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        Ok(Box::new(SqlServerTransaction { conn }))
    }

    async fn get_erd_graph(&self, schema: &str) -> Result<ErdGraph, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;

        let all_cols = crate::connections::sqlserver::list_all_columns(&mut conn, schema)
            .await?;

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
            let fks =
                crate::connections::sqlserver::list_foreign_keys(&mut conn, schema, &table.name)
                    .await?;
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

    async fn import_csv(
        &self,
        database: &str,
        table: &str,
        headers: &[String],
        rows: &[Vec<String>],
        create_table: bool,
    ) -> Result<u64, RowmanceError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;

        let q = |ident: &str| format!("[{}]", ident.replace(']', "]]"));

        if create_table {
            let col_defs: Vec<String> = headers
                .iter()
                .map(|h| format!("{} NVARCHAR(MAX)", q(h)))
                .collect();
            let ddl = format!(
                "IF OBJECT_ID(N'{}.{}', N'U') IS NULL CREATE TABLE {}.{} ({})",
                q(database),
                q(table),
                q(database),
                q(table),
                col_defs.join(", ")
            );
            crate::connections::sqlserver::exec_simple(&mut conn, &ddl).await?;
        }

        let col_list: Vec<String> = headers.iter().map(|h| q(h)).collect();
        let placeholders: Vec<String> = (1..=headers.len()).map(|i| format!("@P{i}")).collect();
        let sql = format!(
            "INSERT INTO {}.{} ({}) VALUES ({})",
            q(database),
            q(table),
            col_list.join(", "),
            placeholders.join(", ")
        );

        let mut inserted = 0u64;
        for row in rows {
            let params: Vec<Box<dyn tiberius::ToSql + Send>> = row
                .iter()
                .map(|s| -> Box<dyn tiberius::ToSql + Send> { Box::new(s.clone()) })
                .collect();
            let params_refs: Vec<&dyn tiberius::ToSql> =
                params.iter().map(|p| p.as_ref() as &dyn tiberius::ToSql).collect();
            conn.execute(sql.as_str(), &params_refs)
                .await
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            inserted += 1;
        }

        Ok(inserted)
    }
}

// ── SqlServerTransaction ──────────────────────────────────────────────────────

pub struct SqlServerTransaction {
    conn: bb8::PooledConnection<'static, bb8_tiberius::ConnectionManager>,
}

#[async_trait]
impl EngineTransaction for SqlServerTransaction {
    async fn execute(
        &mut self,
        sql: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        execute_on_sqlserver_conn(&mut self.conn, sql, page_size, offset).await
    }

    async fn apply_changes(
        &mut self,
        database: &str,
        table: &str,
        updates: &[RowChange],
        inserts: &[HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        apply_all_sqlserver(&mut self.conn, database, table, updates, inserts, deletes).await
    }

    async fn execute_raw(&mut self, sql: &str) -> Result<(), RowmanceError> {
        crate::connections::sqlserver::exec_simple(&mut self.conn, sql).await
    }

    async fn commit(&mut self) -> Result<(), RowmanceError> {
        crate::connections::sqlserver::exec_simple(&mut self.conn, "COMMIT TRANSACTION").await
    }

    async fn rollback(&mut self) -> Result<(), RowmanceError> {
        crate::connections::sqlserver::exec_simple(&mut self.conn, "ROLLBACK TRANSACTION").await
    }

    async fn switch_context(&mut self, database: &str) -> Result<(), RowmanceError> {
        let db_esc = database.replace(']', "]]");
        crate::connections::sqlserver::exec_simple(&mut self.conn, &format!("USE [{db_esc}]"))
            .await
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

async fn execute_on_sqlserver_conn(
    conn: &mut MssqlConn,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> Result<EngineQueryResult, RowmanceError> {
    let exec_sql = sql.trim_end_matches(';');
    let is_select = returns_rows_heuristic_mssql(exec_sql);

    if is_select {
        let paginated = build_pagination_mssql(exec_sql, page_size as u64, offset as u64);
        let mut stream = conn
            .simple_query(paginated.as_str())
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;

        let mut columns: Vec<ColumnMeta> = vec![];
        let mut raw_rows: Vec<tiberius::Row> = vec![];

        while let Some(item) = stream
            .try_next()
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?
        {
            match item {
                tiberius::QueryItem::Metadata(meta) => {
                    if columns.is_empty() {
                        columns = meta
                            .columns()
                            .iter()
                            .map(|c| ColumnMeta {
                                name: c.name().to_owned(),
                                data_type: mssql_column_type_name(c.column_type()),
                                nullable: true,
                                is_primary_key: false,
                                is_foreign_key: false,
                            })
                            .collect();
                    }
                }
                tiberius::QueryItem::Row(row) => {
                    raw_rows.push(row);
                }
            }
        }

        let data: Vec<Vec<serde_json::Value>> = raw_rows
            .into_iter()
            .map(|row| row.into_iter().map(mssql_column_data_to_json).collect())
            .collect();

        Ok(EngineQueryResult {
            columns,
            rows: data,
            affected_rows: None,
        })
    } else {
        let result = conn
            .execute(exec_sql, &[])
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        Ok(EngineQueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: Some(result.total()),
        })
    }
}

fn returns_rows_heuristic_mssql(sql: &str) -> bool {
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;
    match Parser::parse_sql(&MsSqlDialect {}, sql) {
        Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
        Err(_) => returns_rows_heuristic(sql),
    }
}

fn build_pagination_mssql(sql: &str, limit: u64, offset: u64) -> String {
    if limit == 0 {
        return sql.to_string();
    }
    let upper = sql.to_uppercase();
    if upper.contains("FETCH NEXT") || upper.contains("FETCH FIRST") {
        return sql.to_string();
    }
    let trimmed = upper.trim_start();
    if trimmed.starts_with("SELECT TOP") || trimmed.starts_with("WITH ") {
        return sql.to_string();
    }
    if sql_has_top_level_order_by(sql) {
        format!("{sql} OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
    } else {
        format!("{sql} ORDER BY (SELECT NULL) OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
    }
}

async fn apply_all_sqlserver(
    conn: &mut MssqlConn,
    schema: &str,
    table: &str,
    updates: &[RowChange],
    inserts: &[HashMap<String, serde_json::Value>],
    deletes: &[RowDelete],
) -> Result<(u64, u64, u64), RowmanceError> {
    let q = |ident: &str| format!("[{}]", ident.replace(']', "]]"));

    let mut updated = 0u64;
    let mut inserted = 0u64;
    let mut deleted = 0u64;

    // Updates
    for change in updates {
        if change.changes.is_empty() || change.primary_keys.is_empty() {
            continue;
        }
        let mut param_idx: usize = 1;
        let set_clause: Vec<String> = change
            .changes
            .keys()
            .map(|col| {
                let s = format!("{} = @P{}", q(col), param_idx);
                param_idx += 1;
                s
            })
            .collect();
        let where_pairs: Vec<(&String, &serde_json::Value)> = change.primary_keys.iter().collect();
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_bind: Vec<&serde_json::Value> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = @P{}", q(col), param_idx));
                param_idx += 1;
                where_bind.push(val);
            }
        }
        let sql = format!(
            "UPDATE {}.{} SET {} WHERE {}",
            q(schema),
            q(table),
            set_clause.join(", "),
            where_parts.join(" AND ")
        );
        let mut all_bind: Vec<&serde_json::Value> = change.changes.values().collect();
        all_bind.extend(where_bind.iter().copied());
        let params = build_mssql_params(&all_bind);
        let params_refs: Vec<&dyn tiberius::ToSql> =
            params.iter().map(|p| p.as_ref() as &dyn tiberius::ToSql).collect();
        let result = conn
            .execute(sql.as_str(), &params_refs)
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        updated += result.total();
    }

    // Inserts — skip identity columns
    if !inserts.is_empty() {
        let id_sql = "SELECT c.name FROM sys.columns c \
            WHERE c.object_id = OBJECT_ID(N'[' + @P1 + N'].[' + @P2 + N']') \
            AND c.is_identity = 1";
        let identity_cols: std::collections::HashSet<String> = {
            let mut stream = conn
                .query(
                    id_sql,
                    &[
                        &schema as &dyn tiberius::ToSql,
                        &table as &dyn tiberius::ToSql,
                    ],
                )
                .await
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let mut set = std::collections::HashSet::<String>::new();
            while let Some(item) = stream
                .try_next()
                .await
                .map_err(|e| RowmanceError::Pool(e.to_string()))?
            {
                if let tiberius::QueryItem::Row(row) = item {
                    if let Ok(Some(name)) = row.try_get::<&str, _>(0) {
                        set.insert(name.to_owned());
                    }
                }
            }
            set
        };

        for values in inserts {
            if values.is_empty() {
                continue;
            }
            let cols: Vec<(&String, &serde_json::Value)> = values
                .iter()
                .filter(|(c, _)| !identity_cols.contains(c.as_str()))
                .collect();
            if cols.is_empty() {
                continue;
            }
            let col_list: Vec<String> = cols.iter().map(|(c, _)| q(c)).collect();
            let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("@P{}", i)).collect();
            let sql = format!(
                "INSERT INTO {}.{} ({}) VALUES ({})",
                q(schema),
                q(table),
                col_list.join(", "),
                placeholders.join(", ")
            );
            let vals: Vec<&serde_json::Value> = cols.iter().map(|(_, v)| *v).collect();
            let params = build_mssql_params(&vals);
            let params_refs: Vec<&dyn tiberius::ToSql> =
                params.iter().map(|p| p.as_ref() as &dyn tiberius::ToSql).collect();
            conn.execute(sql.as_str(), &params_refs)
                .await
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            inserted += 1;
        }
    }

    // Deletes
    for row_del in deletes {
        if row_del.primary_keys.is_empty() {
            continue;
        }
        let where_pairs: Vec<(&String, &serde_json::Value)> = row_del.primary_keys.iter().collect();
        let mut param_idx = 1usize;
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_bind: Vec<&serde_json::Value> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = @P{}", q(col), param_idx));
                param_idx += 1;
                where_bind.push(val);
            }
        }
        let sql = format!(
            "DELETE FROM {}.{} WHERE {}",
            q(schema),
            q(table),
            where_parts.join(" AND ")
        );
        let params = build_mssql_params(&where_bind);
        let params_refs: Vec<&dyn tiberius::ToSql> =
            params.iter().map(|p| p.as_ref() as &dyn tiberius::ToSql).collect();
        let result = conn
            .execute(sql.as_str(), &params_refs)
            .await
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        deleted += result.total();
    }

    Ok((updated, inserted, deleted))
}

fn build_mssql_params(values: &[&serde_json::Value]) -> Vec<Box<dyn tiberius::ToSql + Send>> {
    values
        .iter()
        .map(|v| -> Box<dyn tiberius::ToSql + Send> {
            match *v {
                serde_json::Value::String(s) => Box::new(s.clone()),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Box::new(i)
                    } else if let Some(f) = n.as_f64() {
                        Box::new(f)
                    } else {
                        Box::new(n.to_string())
                    }
                }
                serde_json::Value::Bool(b) => Box::new(*b),
                _ => Box::new(Option::<String>::None),
            }
        })
        .collect()
}

fn mssql_column_type_name(col_type: tiberius::ColumnType) -> String {
    use tiberius::ColumnType;
    match col_type {
        ColumnType::Null => "null".to_string(),
        ColumnType::Bit => "bit".to_string(),
        ColumnType::Int1 => "tinyint".to_string(),
        ColumnType::Int2 => "smallint".to_string(),
        ColumnType::Int4 => "int".to_string(),
        ColumnType::Int8 => "bigint".to_string(),
        ColumnType::Float4 => "real".to_string(),
        ColumnType::Float8 => "float".to_string(),
        ColumnType::Money => "money".to_string(),
        ColumnType::Money4 => "smallmoney".to_string(),
        ColumnType::Datetime => "datetime".to_string(),
        ColumnType::Datetime2 => "datetime2".to_string(),
        ColumnType::Datetimen => "datetime".to_string(),
        ColumnType::Datetime4 => "smalldatetime".to_string(),
        ColumnType::DatetimeOffsetn => "datetimeoffset".to_string(),
        ColumnType::Daten => "date".to_string(),
        ColumnType::Timen => "time".to_string(),
        ColumnType::Guid => "uniqueidentifier".to_string(),
        ColumnType::Decimaln | ColumnType::Numericn => "decimal".to_string(),
        ColumnType::BigVarBin => "varbinary(max)".to_string(),
        ColumnType::BigBinary => "binary".to_string(),
        ColumnType::BigVarChar => "varchar(max)".to_string(),
        ColumnType::BigChar => "char".to_string(),
        ColumnType::NVarchar => "nvarchar".to_string(),
        ColumnType::NChar => "nchar".to_string(),
        ColumnType::Text => "text".to_string(),
        ColumnType::NText => "ntext".to_string(),
        ColumnType::Image => "image".to_string(),
        ColumnType::Xml => "xml".to_string(),
        ColumnType::Udt => "udt".to_string(),
        ColumnType::SSVariant => "sql_variant".to_string(),
        _ => "unknown".to_string(),
    }
}

fn mssql_column_data_to_json(data: tiberius::ColumnData<'_>) -> serde_json::Value {
    use tiberius::ColumnData;
    match data {
        ColumnData::Bit(v) => v
            .map(serde_json::Value::Bool)
            .unwrap_or(serde_json::Value::Null),
        ColumnData::U8(v) => v
            .map(|n| serde_json::Value::from(n as i64))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::I16(v) => v
            .map(|n| serde_json::Value::from(n as i64))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::I32(v) => v
            .map(|n| serde_json::Value::from(n as i64))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::I64(v) => v
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null),
        ColumnData::F32(v) => v
            .and_then(|f| {
                serde_json::Number::from_f64(f as f64).map(serde_json::Value::Number)
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::F64(v) => v
            .and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::Guid(v) => v
            .map(|g| serde_json::Value::String(g.to_string()))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::String(v) => v
            .map(|s| serde_json::Value::String(s.into_owned()))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::Binary(v) => v
            .map(|b| {
                serde_json::Value::String(
                    b.iter().map(|byte| format!("{:02x}", byte)).collect(),
                )
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::Numeric(v) => v
            .map(|n| serde_json::Value::String(n.to_string()))
            .unwrap_or(serde_json::Value::Null),
        ColumnData::DateTime(v) => v
            .map(|dt| {
                let epoch = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
                let date = epoch + chrono::Duration::days(dt.days() as i64);
                let nanos = dt.seconds_fragments() as i64 * 1_000_000_000 / 300;
                let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                    + chrono::Duration::nanoseconds(nanos);
                serde_json::Value::String(chrono::NaiveDateTime::new(date, time).to_string())
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::SmallDateTime(v) => v
            .map(|dt| {
                let epoch = chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
                let date = epoch + chrono::Duration::days(dt.days() as i64);
                let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
                    dt.seconds_fragments() as u32 * 60,
                    0,
                )
                .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                serde_json::Value::String(chrono::NaiveDateTime::new(date, time).to_string())
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::Time(v) => v
            .map(|t| {
                let nanos = t.increments() as i64 * 10i64.pow(9 - t.scale() as u32);
                let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                    + chrono::Duration::nanoseconds(nanos);
                serde_json::Value::String(time.to_string())
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::Date(v) => v
            .map(|d| {
                let epoch = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                let date = epoch + chrono::Duration::days(d.days() as i64);
                serde_json::Value::String(date.to_string())
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::DateTime2(v) => v
            .map(|dt| {
                let epoch = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                let date = epoch + chrono::Duration::days(dt.date().days() as i64);
                let nanos =
                    dt.time().increments() as i64 * 10i64.pow(9 - dt.time().scale() as u32);
                let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                    + chrono::Duration::nanoseconds(nanos);
                serde_json::Value::String(chrono::NaiveDateTime::new(date, time).to_string())
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::DateTimeOffset(v) => v
            .map(|dto| {
                let epoch = chrono::NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
                let date =
                    epoch + chrono::Duration::days(dto.datetime2().date().days() as i64);
                let nanos = dto.datetime2().time().increments() as i64
                    * 10i64.pow(9 - dto.datetime2().time().scale() as u32);
                let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                    + chrono::Duration::nanoseconds(nanos);
                let naive = chrono::NaiveDateTime::new(date, time);
                let offset_secs = dto.offset() as i32 * 60;
                let fixed_offset = chrono::FixedOffset::east_opt(offset_secs)
                    .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
                let dt = chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(
                    naive - chrono::Duration::seconds(dto.offset() as i64 * 60),
                    fixed_offset,
                );
                serde_json::Value::String(dt.to_rfc3339())
            })
            .unwrap_or(serde_json::Value::Null),
        ColumnData::Xml(v) => v
            .map(|x| serde_json::Value::String(x.as_ref().to_string()))
            .unwrap_or(serde_json::Value::Null),
    }
}

fn sql_has_top_level_order_by(sql: &str) -> bool {
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
            b'[' => {
                i += 1;
                while i < len && bytes[i] != b']' {
                    i += 1;
                }
                if i < len {
                    i += 1;
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
                if depth == 0 && i + 8 <= len {
                    let word = &sql[i..i + 8];
                    if word.eq_ignore_ascii_case("order by") {
                        let before_ok = i == 0
                            || (!bytes[i - 1].is_ascii_alphabetic() && bytes[i - 1] != b'_');
                        let after_ok = bytes
                            .get(i + 8)
                            .is_none_or(|b| b.is_ascii_whitespace() || !b.is_ascii_alphanumeric());
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
        "sqlserver" => Some(DialectInfo {
            identifier_open: "[".into(),
            identifier_close: "]".into(),
            identifier_escape: "]]".into(),
            uses_schema: true,
            db_label: "Schema".into(),
            select_top: true,
            boolean_literals: false,
            uses_ilike: false,
            cast_to_text: "CAST({col} AS NVARCHAR(MAX))".into(),
            supports_user_management: false,
            host_based_users: false,
            supports_roles: false,
            detects_sql_variables: false,
            warns_tx_database_mismatch: false,
            display_name: "SQL Server".into(),
            default_column_type: "INT IDENTITY(1,1)".into(),
            common_column_types: vec![
                "INT", "BIGINT", "SMALLINT", "TINYINT", "INT IDENTITY(1,1)",
                "NVARCHAR(255)", "NVARCHAR(MAX)", "VARCHAR(255)", "NTEXT",
                "DATETIME2", "DATE", "TIME", "FLOAT", "DECIMAL(10,2)", "MONEY", "BIT",
            ].into_iter().map(String::from).collect(),
            supports_auto_increment: false,
            supports_column_comment: false,
            supports_change_column: false,
            supports_rename_column: false,
            uses_foreign_key_keyword: false,
            drop_index_syntax: "on_table_no_schema".into(),
            default_new_column_type: "NVARCHAR(255)".into(),
            uses_database_keyword: false,
            drop_schema_cascade: false,
            is_file_based: false,
            default_port: 1433,
            url_schemes: vec!["sqlserver".into(), "mssql".into()],
            url_template: "sqlserver://{username}:{password}@{host}:{port}/{database}".into(),
            fk_violation: None,
            editor_dialect: "sql".into(),
            explain_format: "sqlserver_xml".into(),
            system_databases: vec![
                "master".into(),
                "model".into(),
                "msdb".into(),
                "tempdb".into(),
            ],
            file_extensions: vec![],
        }),
        _ => None,
    }
}

// ── Pool adapter ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SqlServerPoolAdapter {
    pub pool: MssqlPool,
    pub read_only: bool,
}

#[async_trait]
impl crate::connections::engine::PoolAdapter for SqlServerPoolAdapter {
    async fn disconnect(&self) {
        // bb8 pools are dropped automatically; no explicit close needed.
    }
    async fn ping(&self) -> bool {
        if let Ok(mut conn) = self.pool.get().await {
            conn.simple_query("SELECT 1").await.is_ok()
        } else {
            false
        }
    }
    fn get_engine(&self) -> std::sync::Arc<dyn crate::connections::engine::DatabaseEngine> {
        std::sync::Arc::new(SqlServerEngine { pool: self.pool.clone(), read_only: self.read_only })
    }
}

// ── Pool creation ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn create_pool(
    host: &str,
    port: u16,
    database: &str,
    username: &str,
    password: &str,
    pool_max: u32,
    ssl_enabled: bool,
    ssl_ca_path: Option<&str>,
    read_only: bool,
) -> Result<Box<dyn crate::connections::engine::PoolAdapter>, crate::error::RowmanceError> {
    use tiberius::{AuthMethod, Config, EncryptionLevel};

    let mut config = Config::new();
    config.host(host);
    config.port(port);
    config.database(database);
    config.authentication(AuthMethod::sql_server(username, password));

    config.encryption(EncryptionLevel::Required);
    if !ssl_enabled || ssl_ca_path.is_none() {
        config.trust_cert();
    }

    // Verify credentials with a direct tiberius connection before building the pool.
    // pool.get() always returns bb8::RunError::TimedOut on failure, hiding the real
    // error (e.g. wrong password). A raw connection surfaces the actual tiberius error.
    {
        use tokio_util::compat::TokioAsyncWriteCompatExt;
        let tcp = tokio::net::TcpStream::connect((host, port))
            .await
            .map_err(|e| crate::error::RowmanceError::Pool(
                format!("Cannot reach SQL Server at {host}:{port}: {e}")
            ))?;
        tiberius::Client::connect(config.clone(), tcp.compat_write())
            .await
            .map_err(|e| crate::error::RowmanceError::Pool(e.to_string()))?;
    }

    let manager = bb8_tiberius::ConnectionManager::build(config)
        .map_err(|e| crate::error::RowmanceError::Pool(e.to_string()))?;
    let pool = bb8::Pool::builder()
        .max_size(pool_max)
        .connection_timeout(std::time::Duration::from_secs(10))
        .build(manager)
        .await
        .map_err(|e| crate::error::RowmanceError::Pool(e.to_string()))?;

    Ok(Box::new(SqlServerPoolAdapter { pool, read_only }))
}

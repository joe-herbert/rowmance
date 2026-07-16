/// PostgreSQL engine implementation of the `DatabaseEngine` trait.
use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::Column;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;
use sqlx::TypeInfo;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::connections::engine::{DatabaseEngine, EngineTransaction};
use crate::connections::erd::{build_edges_from_fk_rows, group_into_tables, FkNorm};
use crate::connections::types::{
    BulkColumnRow, ColumnInfo, ColumnMeta, DbUser, ErdColumn, ErdGraph, EngineQueryResult,
    ExplainResult, ForeignKeyInfo, IndexInfo, RowChange, RowDelete, TableInfo,
};
use crate::error::RowmanceError;

pub struct PostgresEngine {
    pub pool: sqlx::PgPool,
}

#[async_trait]
impl DatabaseEngine for PostgresEngine {
    fn quote(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn boolean_literals(&self) -> bool { true }

    fn placeholder(&self, n: usize) -> String {
        format!("${n}")
    }

    fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String {
        if limit == 0 {
            sql.to_string()
        } else {
            format!("{sql} LIMIT {limit} OFFSET {offset}")
        }
    }

    fn parse_returns_rows(&self, sql: &str) -> bool {
        match Parser::parse_sql(&PostgreSqlDialect {}, sql) {
            Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
            Err(_) => returns_rows_heuristic(sql),
        }
    }

    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> {
        crate::connections::postgres::list_databases(&self.pool).await
    }

    async fn list_tables(&self, database: &str) -> Result<Vec<TableInfo>, RowmanceError> {
        crate::connections::postgres::list_tables(&self.pool, database).await
    }

    async fn list_columns(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, RowmanceError> {
        crate::connections::postgres::list_columns(&self.pool, database, table).await
    }

    async fn list_all_columns(&self, database: &str) -> Result<Vec<BulkColumnRow>, RowmanceError> {
        let pairs = crate::connections::postgres::list_all_columns(&self.pool, database).await?;
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
        crate::connections::postgres::list_indexes(&self.pool, database, table).await
    }

    async fn list_foreign_keys(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
        crate::connections::postgres::list_foreign_keys(&self.pool, database, table).await
    }

    async fn count_table(&self, database: &str, table: &str) -> Result<i64, RowmanceError> {
        crate::connections::postgres::count_table(&self.pool, database, table).await
    }

    async fn get_ddl(&self, database: &str, table: &str) -> Result<String, RowmanceError> {
        crate::connections::postgres::get_ddl(&self.pool, database, table).await
    }

    async fn execute(
        &self,
        sql: &str,
        database: Option<&str>,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        if let Some(schema) = database {
            let set_path_sql = format!("SET search_path = {}", self.quote(schema));
            conn.execute(sqlx::raw_sql(&set_path_sql)).await.ok();
        }

        execute_on_pg_conn(&mut conn, sql, page_size, offset).await
    }

    async fn execute_ddl(&self, sql: &str) -> Result<(), RowmanceError> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn count_query_rows(&self, sql: &str, database: Option<&str>) -> Option<i64> {
        let sql_trimmed = sql.trim_end_matches(';');
        let count_sql = format!("SELECT COUNT(*) FROM ({sql_trimmed}) AS _count_query");
        let mut conn = self.pool.acquire().await.ok()?;

        if let Some(schema) = database {
            let set_path_sql = format!("SET search_path = {}", self.quote(schema));
            conn.execute(sqlx::raw_sql(&set_path_sql)).await.ok();
        }

        sqlx::query_scalar::<_, i64>(&count_sql)
            .fetch_optional(&mut *conn)
            .await
            .ok()
            .flatten()
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
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        sqlx::query("BEGIN")
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;

        let result = apply_all_postgres(&mut conn, database, table, updates, inserts, deletes).await;

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
        Ok(Box::new(PostgresTransaction { conn }))
    }

    async fn explain(&self, sql: &str, database: Option<&str>) -> Result<ExplainResult, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        if let Some(schema) = database {
            let quoted = format!("\"{}\"", schema.replace('"', "\"\""));
            sqlx::query(&format!("SET search_path = {quoted}"))
                .execute(&mut *conn)
                .await
                .ok();
        }
        let explain_sql = format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {sql}");
        let rows = sqlx::query(&explain_sql)
            .fetch_all(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;
        let plans: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                row.try_get::<serde_json::Value, _>(0)
                    .unwrap_or(serde_json::Value::Null)
            })
            .collect();
        Ok(ExplainResult {
            raw_json: serde_json::to_string(&plans)
                .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?,
            dialect: "postgres_json".to_string(),
        })
    }

    async fn begin_session(&self) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        Ok(Box::new(PostgresTransaction { conn }))
    }

    async fn get_erd_graph(&self, schema: &str) -> Result<ErdGraph, RowmanceError> {
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
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

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
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        let edges = build_edges_from_fk_rows(fk_rows.into_iter().map(|r| FkNorm {
            constraint_name: r.constraint_name.unwrap_or_default(),
            table_name: r.table_name.unwrap_or_default(),
            column_name: r.column_name.unwrap_or_default(),
            referenced_table_name: r.referenced_table_name.unwrap_or_default(),
            referenced_column_name: r.referenced_column_name.unwrap_or_default(),
        }));

        Ok(ErdGraph { nodes, edges })
    }

    async fn list_users(&self) -> Result<Vec<DbUser>, RowmanceError> {
        #[derive(sqlx::FromRow)]
        struct PgRoleRow {
            username: String,
            rolsuper: bool,
            rolcreatedb: bool,
            rolcreaterole: bool,
            is_locked: bool,
        }
        let rows = sqlx::query_as::<_, PgRoleRow>(
            "SELECT rolname AS username, rolsuper, rolcreatedb, rolcreaterole, \
             NOT rolcanlogin AS is_locked \
             FROM pg_roles WHERE rolcanlogin ORDER BY rolname",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        let users = rows
            .into_iter()
            .map(|r| DbUser {
                username: r.username,
                host: None,
                is_superuser: r.rolsuper,
                can_create_db: r.rolcreatedb,
                can_create_role: r.rolcreaterole,
                is_locked: r.is_locked,
            })
            .collect();
        Ok(users)
    }

    async fn get_user_grants(
        &self,
        username: &str,
        _host: Option<&str>,
    ) -> Result<Vec<String>, RowmanceError> {
        #[derive(sqlx::FromRow)]
        struct PgRoleAttrs {
            rolsuper: bool,
            rolcreatedb: bool,
            rolcreaterole: bool,
            rolbypassrls: bool,
        }
        let attrs = sqlx::query_as::<_, PgRoleAttrs>(
            "SELECT rolsuper, rolcreatedb, rolcreaterole, rolbypassrls \
             FROM pg_roles WHERE rolname = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        let mut grants: Vec<String> = Vec::new();

        if let Some(a) = attrs {
            let super_str = if a.rolsuper { "SUPERUSER" } else { "NOSUPERUSER" };
            let createdb_str = if a.rolcreatedb { "CREATEDB" } else { "NOCREATEDB" };
            let createrole_str = if a.rolcreaterole { "CREATEROLE" } else { "NOCREATEROLE" };
            let bypassrls_str = if a.rolbypassrls { "BYPASSRLS" } else { "NOBYPASSRLS" };
            grants.push(format!(
                "ALTER ROLE \"{username}\" {super_str} {createdb_str} {createrole_str} {bypassrls_str};"
            ));
        }

        #[derive(sqlx::FromRow)]
        struct PgMember {
            member_of: String,
        }
        let memberships = sqlx::query_as::<_, PgMember>(
            "SELECT r.rolname AS member_of \
             FROM pg_auth_members m \
             JOIN pg_roles r ON r.oid = m.roleid \
             JOIN pg_roles u ON u.oid = m.member \
             WHERE u.rolname = $1",
        )
        .bind(username)
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        for m in memberships {
            grants.push(format!("GRANT \"{}\" TO \"{username}\";", m.member_of));
        }

        Ok(grants)
    }

    async fn create_user(
        &self,
        username: &str,
        _host: Option<&str>,
        password: &str,
        is_superuser: bool,
        can_create_db: bool,
        can_create_role: bool,
    ) -> Result<(), RowmanceError> {
        let ep = escape_sql_string(password);
        let mut parts = vec![format!("CREATE USER \"{username}\" WITH PASSWORD '{ep}'")];
        if is_superuser {
            parts.push("SUPERUSER".to_string());
        }
        if can_create_db {
            parts.push("CREATEDB".to_string());
        }
        if can_create_role {
            parts.push("CREATEROLE".to_string());
        }
        let sql = parts.join(" ");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn drop_user(
        &self,
        username: &str,
        _host: Option<&str>,
    ) -> Result<(), RowmanceError> {
        let sql = format!("DROP USER \"{username}\"");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn update_user_password(
        &self,
        username: &str,
        _host: Option<&str>,
        password: &str,
    ) -> Result<(), RowmanceError> {
        let ep = escape_sql_string(password);
        let sql = format!("ALTER USER \"{username}\" WITH PASSWORD '{ep}'");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn rename_user(
        &self,
        username: &str,
        _host: Option<&str>,
        new_username: &str,
        _new_host: Option<&str>,
    ) -> Result<(), RowmanceError> {
        if username != new_username {
            let sql = format!("ALTER ROLE \"{username}\" RENAME TO \"{new_username}\"");
            sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .map(|_| ())
                .map_err(RowmanceError::Database)?;
        }
        Ok(())
    }

    async fn execute_grant(&self, sql: &str) -> Result<(), RowmanceError> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
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
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        // Switch schema context
        {
            let set_path_sql = format!("SET search_path = {}", self.quote(database));
            conn.execute(sqlx::raw_sql(&set_path_sql)).await.ok();
        }

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
        let placeholders: Vec<String> = (1..=col_names.len()).map(|i| format!("${i}")).collect();
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

// ── PostgresTransaction ───────────────────────────────────────────────────────

pub struct PostgresTransaction {
    conn: sqlx::pool::PoolConnection<sqlx::Postgres>,
}

#[async_trait]
impl EngineTransaction for PostgresTransaction {
    async fn execute(
        &mut self,
        sql: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        execute_on_pg_conn(&mut self.conn, sql, page_size, offset).await
    }

    async fn apply_changes(
        &mut self,
        database: &str,
        table: &str,
        updates: &[RowChange],
        inserts: &[std::collections::HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        apply_all_postgres(&mut self.conn, database, table, updates, inserts, deletes).await
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

    async fn switch_context(&mut self, database: &str) -> Result<(), RowmanceError> {
        let quoted = format!("\"{}\"", database.replace('"', "\"\""));
        sqlx::query(&format!("SET search_path = {quoted}"))
            .execute(&mut *self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

async fn execute_on_pg_conn(
    conn: &mut sqlx::postgres::PgConnection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> Result<EngineQueryResult, RowmanceError> {
    let exec_sql = sql.trim_end_matches(';');

    if returns_rows_heuristic_pg(exec_sql) {
        let limit = page_size as u64;
        let off = offset as u64;
        let paginated = if limit == 0 {
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
            .map(|row| (0..row.len()).map(|i| pg_value_to_json(row, i)).collect())
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

fn returns_rows_heuristic_pg(sql: &str) -> bool {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;
    match Parser::parse_sql(&PostgreSqlDialect {}, sql) {
        Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
        Err(_) => returns_rows_heuristic(sql),
    }
}

async fn apply_all_postgres(
    conn: &mut sqlx::postgres::PgConnection,
    database: &str,
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
        let mut param_idx: usize = 1;
        let set_clause: Vec<String> = change
            .changes
            .keys()
            .map(|col| {
                let s = format!("{} = ${}", q(col), param_idx);
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
                where_parts.push(format!("{} = ${}", q(col), param_idx));
                param_idx += 1;
                where_bind.push(val);
            }
        }
        let sql = format!(
            "UPDATE {}.{} SET {} WHERE {}",
            q(database),
            q(table),
            set_clause.join(", "),
            where_parts.join(" AND ")
        );
        let mut query = sqlx::query(&sql);
        for val in change.changes.values() {
            query = bind_pg_value(query, val);
        }
        for val in &where_bind {
            query = bind_pg_value(query, val);
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
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "INSERT INTO {}.{} ({}) VALUES ({})",
            q(database),
            q(table),
            col_list.join(", "),
            placeholders.join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (_, val) in &cols {
            query = bind_pg_value(query, val);
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
        let mut param_idx = 1usize;
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_bind: Vec<&serde_json::Value> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = ${}", q(col), param_idx));
                param_idx += 1;
                where_bind.push(val);
            }
        }
        let sql = format!(
            "DELETE FROM {}.{} WHERE {}",
            q(database),
            q(table),
            where_parts.join(" AND ")
        );
        let mut query = sqlx::query(&sql);
        for val in &where_bind {
            query = bind_pg_value(query, val);
        }
        deleted += query
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?
            .rows_affected();
    }

    Ok((updated, inserted, deleted))
}

fn pg_value_to_json(row: &sqlx::postgres::PgRow, idx: usize) -> serde_json::Value {
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return v
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
        return v
            .map(|i| serde_json::Value::from(i as i64))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i16>, _>(idx) {
        return v
            .map(|i| serde_json::Value::from(i as i64))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return v
            .and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f32>, _>(idx) {
        return v
            .and_then(|f| serde_json::Number::from_f64(f as f64).map(serde_json::Value::Number))
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
    if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(idx) {
        return v
            .map(|dt| serde_json::Value::String(dt.to_rfc3339()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(idx) {
        return v
            .map(|dt| serde_json::Value::String(dt.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<chrono::NaiveDate>, _>(idx) {
        return v
            .map(|d| serde_json::Value::String(d.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<chrono::NaiveTime>, _>(idx) {
        return v
            .map(|t| serde_json::Value::String(t.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<
        sqlx::postgres::types::PgTimeTz<chrono::NaiveTime, chrono::FixedOffset>,
    >, _>(idx)
    {
        return v
            .map(|tz| {
                let offset = tz.offset.local_minus_utc();
                let (sign, abs_secs) = if offset >= 0 {
                    ('+', offset as u32)
                } else {
                    ('-', (-offset) as u32)
                };
                serde_json::Value::String(format!(
                    "{}{}{:02}:{:02}",
                    tz.time.format("%H:%M:%S%.f"),
                    sign,
                    abs_secs / 3600,
                    (abs_secs % 3600) / 60,
                ))
            })
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<sqlx::postgres::types::PgInterval>, _>(idx) {
        return v
            .map(|interval| {
                let mut parts = Vec::new();
                if interval.months != 0 {
                    parts.push(format!("{} months", interval.months));
                }
                if interval.days != 0 {
                    parts.push(format!("{} days", interval.days));
                }
                if interval.microseconds != 0 {
                    parts.push(format!("{} μs", interval.microseconds));
                }
                serde_json::Value::String(if parts.is_empty() {
                    "0".to_string()
                } else {
                    parts.join(" ")
                })
            })
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(idx) {
        return v
            .map(|j| serde_json::Value::String(j.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    serde_json::Value::Null
}

fn bind_pg_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        serde_json::Value::String(s) => query.bind(s.clone()),
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
    use crate::connections::types::{DialectInfo, FkViolationHint};
    match db_type {
        "postgres" => Some(DialectInfo {
            identifier_open: "\"".into(),
            identifier_close: "\"".into(),
            identifier_escape: "\"\"".into(),
            uses_schema: true,
            db_label: "Schema".into(),
            select_top: false,
            boolean_literals: true,
            uses_ilike: true,
            cast_to_text: "CAST({col} AS TEXT)".into(),
            supports_user_management: true,
            host_based_users: false,
            supports_roles: true,
            detects_sql_variables: false,
            warns_tx_database_mismatch: false,
            display_name: "PostgreSQL".into(),
            default_column_type: "SERIAL".into(),
            common_column_types: vec![
                "INTEGER", "BIGINT", "SMALLINT",
                "VARCHAR(255)", "TEXT",
                "TIMESTAMP", "DATE", "REAL", "NUMERIC(10,2)",
                "BOOLEAN", "JSON", "JSONB", "UUID", "SERIAL", "BIGSERIAL",
            ].into_iter().map(String::from).collect(),
            supports_auto_increment: false,
            supports_column_comment: false,
            supports_change_column: false,
            supports_rename_column: true,
            uses_foreign_key_keyword: false,
            drop_index_syntax: "schema_qualified".into(),
            default_new_column_type: "TEXT".into(),
            uses_database_keyword: false,
            drop_schema_cascade: true,
            is_file_based: false,
            default_port: 5432,
            url_schemes: vec!["postgres".into(), "postgresql".into()],
            url_template: "postgres://{username}:{password}@{host}:{port}/{database}".into(),
            fk_violation: Some(FkViolationHint {
                detect: "violates foreign key constraint".into(),
                table_pattern: r#"on table "([^"]+)""#.into(),
                column_pair_pattern: None,
                column_value_pattern: Some(
                    r"Key \(([^)]+)\)=\(([^)]+)\) is still referenced from".into()
                ),
            }),
            editor_dialect: "postgresql".into(),
            explain_format: "postgres_json".into(),
            system_databases: vec![],
            file_extensions: vec![],
        }),
        _ => None,
    }
}

// ── Pool adapter ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PostgresPoolAdapter {
    pub pool: sqlx::PgPool,
}

#[async_trait]
impl crate::connections::engine::PoolAdapter for PostgresPoolAdapter {
    async fn disconnect(&self) { self.pool.close().await; }
    async fn ping(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
    fn get_engine(&self) -> std::sync::Arc<dyn crate::connections::engine::DatabaseEngine> {
        std::sync::Arc::new(PostgresEngine { pool: self.pool.clone() })
    }
}

// ── Pool creation ─────────────────────────────────────────────────────────────

/// Reset Postgres connection to the given schema using the text protocol.
async fn pg_reset_schema(conn: &mut sqlx::postgres::PgConnection, set_path_sql: &str) -> bool {
    use sqlx::Executor as _;
    conn.execute(sqlx::raw_sql(set_path_sql)).await.is_ok()
}

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
    ssl_cert_path: Option<&str>,
    ssl_key_path: Option<&str>,
    read_only: bool,
) -> Result<Box<dyn crate::connections::engine::PoolAdapter>, crate::error::RowmanceError> {
    use std::path::Path;
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};

    let mut opts = PgConnectOptions::new()
        .host(host)
        .port(port)
        .database(database)
        .username(username);
    if !password.is_empty() {
        opts = opts.password(password);
    }

    if ssl_enabled {
        let ssl_mode = if ssl_ca_path.is_some() {
            PgSslMode::VerifyCa
        } else {
            PgSslMode::Require
        };
        opts = opts.ssl_mode(ssl_mode);

        if let Some(ca) = ssl_ca_path {
            opts = opts.ssl_root_cert(Path::new(ca));
        }
        if let Some(cert) = ssl_cert_path {
            opts = opts.ssl_client_cert(Path::new(cert));
        }
        if let Some(key) = ssl_key_path {
            opts = opts.ssl_client_key(Path::new(key));
        }
    }

    if read_only {
        opts = opts.options([("default_transaction_read_only", "on")]);
    }

    let schema_esc = database.replace('\'', "''");
    let set_path_sql: &'static str =
        Box::leak(format!("SET search_path = '{}'", schema_esc).into_boxed_str());
    let p = PgPoolOptions::new()
        .min_connections(0)
        .max_connections(pool_max)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .test_before_acquire(true)
        .after_release(move |conn, _meta| {
            Box::pin(async move { Ok(pg_reset_schema(conn, set_path_sql).await) })
        })
        .connect_with(opts)
        .await?;
    // Verify credentials and warm up one idle connection.
    p.acquire().await?;
    Ok(Box::new(PostgresPoolAdapter { pool: p }))
}

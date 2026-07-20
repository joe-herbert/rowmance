/// MySQL/MariaDB engine implementation of the `DatabaseEngine` trait.
use std::collections::HashMap;

use async_trait::async_trait;
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
use sqlx::Column;
use sqlx::Executor;
use sqlx::Row;
use sqlx::Statement;
use sqlx::TypeInfo;

use crate::connections::engine::{DatabaseEngine, EngineTransaction};
use crate::connections::erd::{build_edges_from_fk_rows, group_into_tables, FkNorm};
use crate::connections::types::{
    BulkColumnRow, CapabilityStatus, ColumnInfo, ColumnMeta, DbUser, EngineQueryResult, ErdColumn,
    ErdGraph, ExplainResult, ForeignKeyInfo, IndexInfo, LockInfo, ProcessInfo, RowChange,
    RowDelete, ScheduledJob, ServerAdminCapabilityFlags, ServerStatus, ServerVariable, TableInfo,
    VarScope,
};
use crate::error::RowmanceError;

pub struct MySqlEngine {
    pub pool: sqlx::MySqlPool,
    pub read_only: bool,
}

#[async_trait]
impl DatabaseEngine for MySqlEngine {
    fn quote(&self, ident: &str) -> String {
        format!("`{}`", ident.replace('`', "``"))
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
        match Parser::parse_sql(&MySqlDialect {}, sql) {
            Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
            Err(_) => returns_rows_heuristic(sql),
        }
    }

    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> {
        crate::connections::mysql::list_databases(&self.pool).await
    }

    async fn list_tables(
        &self,
        database: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<TableInfo>, RowmanceError> {
        crate::connections::mysql::list_tables(&self.pool, database).await
    }

    async fn list_columns(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<ColumnInfo>, RowmanceError> {
        crate::connections::mysql::list_columns(&self.pool, database, table).await
    }

    async fn list_all_columns(
        &self,
        database: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<BulkColumnRow>, RowmanceError> {
        let pairs = crate::connections::mysql::list_all_columns(&self.pool, database).await?;
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
        crate::connections::mysql::list_indexes(&self.pool, database, table).await
    }

    async fn list_foreign_keys(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
        crate::connections::mysql::list_foreign_keys(&self.pool, database, table).await
    }

    async fn count_table(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<i64, RowmanceError> {
        crate::connections::mysql::count_table(&self.pool, database, table).await
    }

    async fn get_ddl(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<String, RowmanceError> {
        crate::connections::mysql::get_ddl(&self.pool, database, table).await
    }

    async fn execute(
        &self,
        sql: &str,
        database: Option<&str>,
        _instance_db: Option<&str>,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;

        if let Some(db) = database {
            let db_esc = db.replace('`', "``");
            conn.execute(sqlx::raw_sql(&format!("USE `{}`", db_esc)))
                .await
                .ok();
        }

        execute_on_mysql_conn(&mut conn, sql, page_size, offset).await
    }

    async fn execute_ddl(&self, sql: &str) -> Result<(), RowmanceError> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn count_query_rows(
        &self,
        sql: &str,
        database: Option<&str>,
        _instance_db: Option<&str>,
    ) -> Option<i64> {
        let sql_trimmed = sql.trim_end_matches(';');
        let count_sql = format!("SELECT COUNT(*) FROM ({sql_trimmed}) AS _count_query");
        let mut conn = self.pool.acquire().await.ok()?;

        if let Some(db) = database {
            let db_esc = db.replace('`', "``");
            conn.execute(sqlx::raw_sql(&format!("USE `{}`", db_esc)))
                .await
                .ok();
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

        conn.execute(sqlx::raw_sql("BEGIN"))
            .await
            .map_err(RowmanceError::Database)?;

        let result = apply_all_mysql(&mut conn, database, table, updates, inserts, deletes).await;

        match result {
            Ok(counts) => {
                conn.execute(sqlx::raw_sql("COMMIT"))
                    .await
                    .map_err(RowmanceError::Database)?;
                Ok(counts)
            }
            Err(e) => {
                conn.execute(sqlx::raw_sql("ROLLBACK")).await.ok();
                Err(e)
            }
        }
    }

    async fn ping(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn begin_transaction(
        &self,
        database: Option<&str>,
    ) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        use sqlx::ConnectOptions;
        let opts = (*self.pool.connect_options()).clone();
        let opts = if let Some(db) = database {
            opts.database(db)
        } else {
            opts
        };
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        sqlx::query("SET autocommit = 0")
            .execute(&mut conn)
            .await
            .map_err(RowmanceError::Database)?;
        Ok(Box::new(MySqlTransaction { conn }))
    }

    async fn explain(
        &self,
        sql: &str,
        database: Option<&str>,
        _instance_db: Option<&str>,
    ) -> Result<ExplainResult, RowmanceError> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        if self.read_only {
            sqlx::query("SET SESSION TRANSACTION READ ONLY")
                .execute(&mut *conn)
                .await
                .map_err(RowmanceError::Database)?;
        }
        if let Some(db) = database {
            let db_esc = db.replace('`', "``");
            conn.execute(sqlx::raw_sql(&format!("USE `{}`", db_esc)))
                .await
                .ok();
        }
        let explain_sql = format!("EXPLAIN FORMAT=JSON {sql}");
        let rows = sqlx::query(&explain_sql)
            .fetch_all(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?;
        let raw = rows
            .first()
            .and_then(|row| row.try_get::<String, _>(0).ok())
            .unwrap_or_default();
        Ok(ExplainResult {
            raw_json: raw,
            dialect: "mysql_json".to_string(),
        })
    }

    async fn begin_session(&self) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        use sqlx::ConnectOptions;
        let conn = (*self.pool.connect_options())
            .clone()
            .connect()
            .await
            .map_err(|e| RowmanceError::ConnectionNotFound(e.to_string()))?;
        Ok(Box::new(MySqlTransaction { conn }))
    }

    async fn get_erd_graph(
        &self,
        database: &str,
        _instance_db: Option<&str>,
    ) -> Result<ErdGraph, RowmanceError> {
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
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

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
        use sqlx::Row;
        let rows = sqlx::query(
            "SELECT CAST(User AS CHAR) AS username, CAST(Host AS CHAR) AS host, \
             Super_priv, Create_priv, \
             IFNULL(account_locked, 'N') AS account_locked \
             FROM mysql.user ORDER BY User, Host",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        let users = rows
            .into_iter()
            .map(|row| {
                let username: String = row.try_get("username").unwrap_or_default();
                let host: Option<String> = row.try_get("host").ok();
                let super_priv: Option<String> = row.try_get("Super_priv").ok();
                let create_priv: Option<String> = row.try_get("Create_priv").ok();
                let account_locked: Option<String> = row.try_get("account_locked").ok();
                DbUser {
                    username,
                    host,
                    is_superuser: super_priv.as_deref() == Some("Y"),
                    can_create_db: create_priv.as_deref() == Some("Y"),
                    can_create_role: false,
                    is_locked: account_locked.as_deref() == Some("Y"),
                }
            })
            .collect();
        Ok(users)
    }

    async fn get_user_grants(
        &self,
        username: &str,
        host: Option<&str>,
    ) -> Result<Vec<String>, RowmanceError> {
        use sqlx::Row;
        let h = host.unwrap_or("%");
        let eu = escape_sql_string(username);
        let eh = escape_sql_string(h);
        let sql = format!("SHOW GRANTS FOR '{eu}'@'{eh}'");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;
        let grants = rows
            .into_iter()
            .map(|row| row.try_get::<String, _>(0).unwrap_or_default())
            .collect();
        Ok(grants)
    }

    async fn create_user(
        &self,
        username: &str,
        host: Option<&str>,
        password: &str,
        _is_superuser: bool,
        _can_create_db: bool,
        _can_create_role: bool,
    ) -> Result<(), RowmanceError> {
        let h = host.unwrap_or("%");
        let eu = escape_sql_string(username);
        let eh = escape_sql_string(h);
        let ep = escape_sql_string(password);
        let sql = format!("CREATE USER '{eu}'@'{eh}' IDENTIFIED BY '{ep}'");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn drop_user(&self, username: &str, host: Option<&str>) -> Result<(), RowmanceError> {
        let h = host.unwrap_or("%");
        let eu = escape_sql_string(username);
        let eh = escape_sql_string(h);
        let sql = format!("DROP USER '{eu}'@'{eh}'");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn update_user_password(
        &self,
        username: &str,
        host: Option<&str>,
        password: &str,
    ) -> Result<(), RowmanceError> {
        let h = host.unwrap_or("%");
        let eu = escape_sql_string(username);
        let eh = escape_sql_string(h);
        let ep = escape_sql_string(password);
        let sql = format!("ALTER USER '{eu}'@'{eh}' IDENTIFIED BY '{ep}'");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn rename_user(
        &self,
        username: &str,
        host: Option<&str>,
        new_username: &str,
        new_host: Option<&str>,
    ) -> Result<(), RowmanceError> {
        let h = host.unwrap_or("%");
        let nh = new_host.unwrap_or("%");
        let eu = escape_sql_string(username);
        let eh = escape_sql_string(h);
        let enu = escape_sql_string(new_username);
        let enh = escape_sql_string(nh);
        let sql = format!("RENAME USER '{eu}'@'{eh}' TO '{enu}'@'{enh}'");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
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

        // Switch to target database
        {
            let db_esc = database.replace('`', "``");
            conn.execute(sqlx::raw_sql(&format!("USE `{}`", db_esc)))
                .await
                .ok();
        }

        if create_table {
            let col_defs: Vec<String> = headers
                .iter()
                .map(|h| format!("`{}` TEXT", h.replace('`', "``")))
                .collect();
            let ddl = format!(
                "CREATE TABLE IF NOT EXISTS `{}` ({})",
                table.replace('`', "``"),
                col_defs.join(", ")
            );
            sqlx::query(&ddl)
                .execute(&mut *conn)
                .await
                .map_err(RowmanceError::Database)?;
        }

        let col_names: Vec<String> = headers
            .iter()
            .map(|h| format!("`{}`", h.replace('`', "``")))
            .collect();
        let placeholders: Vec<&str> = headers.iter().map(|_| "?").collect();
        let sql = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            table.replace('`', "``"),
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

    async fn probe_server_admin_capabilities(
        &self,
    ) -> Result<ServerAdminCapabilityFlags, RowmanceError> {
        let process_list = match sqlx::query("SELECT 1 FROM information_schema.PROCESSLIST LIMIT 1")
            .fetch_optional(&self.pool)
            .await
        {
            Ok(_) => CapabilityStatus::Supported,
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("access denied") || msg.contains("denied") {
                    CapabilityStatus::InsufficientPrivileges
                } else {
                    CapabilityStatus::NotSupported
                }
            }
        };

        Ok(ServerAdminCapabilityFlags {
            process_list,
            kill_session: CapabilityStatus::Supported,
            cancel_session: CapabilityStatus::NotSupported,
            server_status: CapabilityStatus::Supported,
            variables: CapabilityStatus::Supported,
            set_variable: CapabilityStatus::Supported,
            scheduled_jobs: CapabilityStatus::Supported,
            locks: CapabilityStatus::Supported,
            innodb_status: CapabilityStatus::Supported,
            vacuum_status: CapabilityStatus::NotSupported,
        })
    }

    async fn list_processes(&self) -> Result<Vec<ProcessInfo>, RowmanceError> {
        let rows = sqlx::query(
            "SELECT ID, USER, HOST, DB, COMMAND, TIME, STATE, INFO \
             FROM information_schema.PROCESSLIST",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        Ok(rows
            .iter()
            .map(|r| ProcessInfo {
                id: r.get::<u64, _>("ID").to_string(),
                user: r.get("USER"),
                host: r.get("HOST"),
                database: r.get("DB"),
                command: r.get("COMMAND"),
                time_seconds: r.try_get::<u64, _>("TIME").ok(),
                state: r.get("STATE"),
                info: r.get("INFO"),
                can_kill: true,
                can_cancel: false,
            })
            .collect())
    }

    async fn kill_session(&self, session_id: &str) -> Result<(), RowmanceError> {
        let id: u64 = session_id
            .parse()
            .map_err(|_| RowmanceError::ConnectionNotFound("Invalid session id".to_string()))?;
        sqlx::query(&format!("KILL CONNECTION {id}"))
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn get_server_status(&self) -> Result<ServerStatus, RowmanceError> {
        let version_row = sqlx::query("SELECT VERSION() AS v")
            .fetch_one(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;
        let version: String = version_row.get("v");

        let status_rows = sqlx::query("SHOW GLOBAL STATUS WHERE Variable_name IN ('Uptime', 'Threads_connected', 'Questions', 'Innodb_buffer_pool_read_requests', 'Innodb_buffer_pool_reads')")
            .fetch_all(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;

        let mut status_map = std::collections::HashMap::<String, String>::new();
        for row in &status_rows {
            let k: String = row.get("Variable_name");
            let v: String = row.get("Value");
            status_map.insert(k, v);
        }

        let max_conn_row = sqlx::query("SHOW VARIABLES LIKE 'max_connections'")
            .fetch_optional(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;
        let connections_max = max_conn_row
            .as_ref()
            .and_then(|r| r.get::<Option<String>, _>("Value"))
            .and_then(|v| v.parse::<u64>().ok());

        let uptime_seconds = status_map
            .get("Uptime")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let connections_current = status_map
            .get("Threads_connected")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let qps = if uptime_seconds > 0 {
            status_map
                .get("Questions")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|q| q / uptime_seconds as f64)
        } else {
            None
        };

        let buf_requests = status_map
            .get("Innodb_buffer_pool_read_requests")
            .and_then(|v| v.parse::<f64>().ok());
        let buf_reads = status_map
            .get("Innodb_buffer_pool_reads")
            .and_then(|v| v.parse::<f64>().ok());
        let cache_hit_ratio = match (buf_requests, buf_reads) {
            (Some(req), Some(rd)) if req > 0.0 => Some((req - rd) / req * 100.0),
            _ => None,
        };

        Ok(ServerStatus {
            version,
            uptime_seconds,
            connections_current,
            connections_max,
            queries_per_second: qps,
            cache_hit_ratio,
            extra: std::collections::HashMap::new(),
        })
    }

    async fn list_variables(&self) -> Result<Vec<ServerVariable>, RowmanceError> {
        let global_rows = sqlx::query("SHOW GLOBAL VARIABLES")
            .fetch_all(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;

        let session_rows = sqlx::query("SHOW SESSION VARIABLES")
            .fetch_all(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;

        let mut global_map = std::collections::HashMap::<String, String>::new();
        for row in &global_rows {
            let k: String = row.get("Variable_name");
            let v: String = row.get("Value");
            global_map.insert(k, v);
        }

        let mut session_map = std::collections::HashMap::<String, String>::new();
        for row in &session_rows {
            let k: String = row.get("Variable_name");
            let v: String = row.get("Value");
            session_map.insert(k, v);
        }

        let mut vars: Vec<ServerVariable> = Vec::new();
        for (name, value) in &global_map {
            let in_session = session_map.contains_key(name.as_str());
            let scope = if in_session {
                VarScope::Both
            } else {
                VarScope::Global
            };
            vars.push(ServerVariable {
                name: name.clone(),
                value: value.clone(),
                scope,
                is_dynamic: true,
                restart_required: false,
                description: None,
                data_type: None,
            });
        }
        for (name, value) in &session_map {
            if !global_map.contains_key(name.as_str()) {
                vars.push(ServerVariable {
                    name: name.clone(),
                    value: value.clone(),
                    scope: VarScope::Session,
                    is_dynamic: true,
                    restart_required: false,
                    description: None,
                    data_type: None,
                });
            }
        }

        vars.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(vars)
    }

    async fn set_variable(
        &self,
        name: &str,
        value: &str,
        scope: VarScope,
    ) -> Result<(), RowmanceError> {
        let scope_str = match scope {
            VarScope::Session => "SESSION",
            VarScope::Global | VarScope::Both => "GLOBAL",
        };
        let en = name.replace('`', "``");
        let ev = escape_sql_string(value);
        let sql = format!("SET {scope_str} `{en}` = '{ev}'");
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn list_locks(&self) -> Result<Vec<LockInfo>, RowmanceError> {
        let rows_result = sqlx::query(
            "SELECT \
                LOCK_ID, ENGINE_TRANSACTION_ID, LOCK_MODE, LOCK_TYPE, OBJECT_NAME \
             FROM performance_schema.data_locks \
             WHERE LOCK_STATUS = 'WAITING' OR OBJECT_NAME IS NOT NULL \
             LIMIT 200",
        )
        .fetch_all(&self.pool)
        .await;

        match rows_result {
            Ok(rows) => Ok(rows
                .iter()
                .map(|r| LockInfo {
                    lock_id: r.try_get::<String, _>("LOCK_ID").unwrap_or_default(),
                    blocker_session_id: None,
                    waiting_session_id: r
                        .try_get::<Option<u64>, _>("ENGINE_TRANSACTION_ID")
                        .ok()
                        .flatten()
                        .map(|v| v.to_string()),
                    lock_type: r.try_get::<String, _>("LOCK_TYPE").unwrap_or_default(),
                    lock_mode: r.try_get::<String, _>("LOCK_MODE").unwrap_or_default(),
                    object_name: r.try_get("OBJECT_NAME").ok(),
                    duration_ms: None,
                })
                .collect()),
            Err(_) => {
                let rows2 = sqlx::query(
                    "SELECT \
                        r.trx_id AS waiting_trx_id, \
                        r.trx_mysql_thread_id AS waiting_thread, \
                        b.trx_id AS blocking_trx_id, \
                        b.trx_mysql_thread_id AS blocking_thread, \
                        r.trx_query AS waiting_query \
                     FROM information_schema.INNODB_TRX r \
                     INNER JOIN information_schema.INNODB_TRX b ON b.trx_id != r.trx_id \
                     WHERE r.trx_wait_started IS NOT NULL \
                     LIMIT 200",
                )
                .fetch_all(&self.pool)
                .await
                .map_err(RowmanceError::Database)?;

                Ok(rows2
                    .iter()
                    .map(|r| LockInfo {
                        lock_id: r.try_get::<String, _>("waiting_trx_id").unwrap_or_default(),
                        blocker_session_id: r
                            .try_get::<Option<u64>, _>("blocking_thread")
                            .ok()
                            .flatten()
                            .map(|v| v.to_string()),
                        waiting_session_id: r
                            .try_get::<Option<u64>, _>("waiting_thread")
                            .ok()
                            .flatten()
                            .map(|v| v.to_string()),
                        lock_type: "InnoDB".to_string(),
                        lock_mode: "LOCK".to_string(),
                        object_name: None,
                        duration_ms: None,
                    })
                    .collect())
            }
        }
    }

    async fn list_scheduled_jobs(&self) -> Result<Vec<ScheduledJob>, RowmanceError> {
        let rows = sqlx::query(
            "SELECT EVENT_NAME, EVENT_SCHEMA, STATUS, INTERVAL_VALUE, INTERVAL_FIELD, \
                    LAST_EXECUTED, EXECUTE_AT, EVENT_DEFINITION \
             FROM information_schema.EVENTS \
             ORDER BY EVENT_SCHEMA, EVENT_NAME",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RowmanceError::Database)?;

        Ok(rows
            .iter()
            .map(|r| {
                let schema: String = r.try_get("EVENT_SCHEMA").unwrap_or_default();
                let name: String = r.try_get("EVENT_NAME").unwrap_or_default();
                let status: String = r.try_get("STATUS").unwrap_or_default();
                let interval_value: Option<String> = r.try_get("INTERVAL_VALUE").ok().flatten();
                let interval_field: Option<String> = r.try_get("INTERVAL_FIELD").ok().flatten();
                let schedule = match (interval_value, interval_field) {
                    (Some(v), Some(f)) => format!("EVERY {v} {f}"),
                    _ => "ONE TIME".to_string(),
                };
                let last_run: Option<String> = r.try_get("LAST_EXECUTED").ok().flatten();
                let next_run: Option<String> = r.try_get("EXECUTE_AT").ok().flatten();
                let body: Option<String> = r.try_get("EVENT_DEFINITION").ok();
                ScheduledJob {
                    id: format!("{schema}.{name}"),
                    name: format!("{schema}.{name}"),
                    schedule,
                    enabled: status == "ENABLED",
                    last_run,
                    next_run,
                    body,
                }
            })
            .collect())
    }

    async fn get_innodb_status(&self) -> Result<String, RowmanceError> {
        let row = sqlx::query("SHOW ENGINE INNODB STATUS")
            .fetch_one(&self.pool)
            .await
            .map_err(RowmanceError::Database)?;
        let status: String = row.get("Status");
        Ok(status)
    }
}

// ── MySqlTransaction ──────────────────────────────────────────────────────────

pub struct MySqlTransaction {
    conn: sqlx::mysql::MySqlConnection,
}

#[async_trait]
impl EngineTransaction for MySqlTransaction {
    async fn execute(
        &mut self,
        sql: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        execute_on_mysql_conn(&mut self.conn, sql, page_size, offset).await
    }

    async fn apply_changes(
        &mut self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
        updates: &[RowChange],
        inserts: &[std::collections::HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        apply_all_mysql(&mut self.conn, database, table, updates, inserts, deletes).await
    }

    async fn execute_raw(&mut self, sql: &str) -> Result<(), RowmanceError> {
        sqlx::query(sql)
            .execute(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn commit(&mut self) -> Result<(), RowmanceError> {
        sqlx::query("COMMIT")
            .execute(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)?;
        sqlx::query("SET autocommit = 1")
            .execute(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn rollback(&mut self) -> Result<(), RowmanceError> {
        sqlx::query("ROLLBACK")
            .execute(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)?;
        sqlx::query("SET autocommit = 1")
            .execute(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }

    async fn switch_context(&mut self, database: &str) -> Result<(), RowmanceError> {
        let db_esc = database.replace('`', "``");
        self.conn
            .execute(sqlx::raw_sql(&format!("USE `{}`", db_esc)))
            .await
            .map(|_| ())
            .map_err(RowmanceError::Database)
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

async fn execute_on_mysql_conn(
    conn: &mut sqlx::mysql::MySqlConnection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> Result<EngineQueryResult, RowmanceError> {
    let exec_sql = sql.trim_end_matches(';');

    if returns_rows_heuristic_mysql(exec_sql) {
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
                    data_type: format_mysql_type(c.type_info()),
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
                        data_type: format_mysql_type(c.type_info()),
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
                    .map(|i| mysql_value_to_json(row, i))
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

fn returns_rows_heuristic_mysql(sql: &str) -> bool {
    use sqlparser::dialect::MySqlDialect;
    use sqlparser::parser::Parser;
    match Parser::parse_sql(&MySqlDialect {}, sql) {
        Ok(stmts) => stmts.first().map(stmt_returns_rows).unwrap_or(false),
        Err(_) => returns_rows_heuristic(sql),
    }
}

async fn apply_all_mysql(
    conn: &mut sqlx::mysql::MySqlConnection,
    database: &str,
    table: &str,
    updates: &[RowChange],
    inserts: &[HashMap<String, serde_json::Value>],
    deletes: &[RowDelete],
) -> Result<(u64, u64, u64), RowmanceError> {
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
            .map(|col| format!("`{}` = ?", col.replace('`', "``")))
            .collect();
        let where_pairs: Vec<(&String, &serde_json::Value)> = change.primary_keys.iter().collect();
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_bind: Vec<&serde_json::Value> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("`{}` IS NULL", col.replace('`', "``")));
            } else {
                where_parts.push(format!("`{}` = ?", col.replace('`', "``")));
                where_bind.push(val);
            }
        }
        let sql = format!(
            "UPDATE `{}`.`{}` SET {} WHERE {} LIMIT 1",
            database.replace('`', "``"),
            table.replace('`', "``"),
            set_clause.join(", "),
            where_parts.join(" AND ")
        );
        let mut q = sqlx::query(&sql);
        for val in change.changes.values() {
            q = bind_mysql_value(q, val);
        }
        for val in &where_bind {
            q = bind_mysql_value(q, val);
        }
        updated += q
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
        let col_list: Vec<String> = cols
            .iter()
            .map(|(c, _)| format!("`{}`", c.replace('`', "``")))
            .collect();
        let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
        let sql = format!(
            "INSERT INTO `{}`.`{}` ({}) VALUES ({})",
            database.replace('`', "``"),
            table.replace('`', "``"),
            col_list.join(", "),
            placeholders.join(", ")
        );
        let mut q = sqlx::query(&sql);
        for (_, val) in &cols {
            q = bind_mysql_value(q, val);
        }
        q.execute(&mut *conn)
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
                where_parts.push(format!("`{}` IS NULL", col.replace('`', "``")));
            } else {
                where_parts.push(format!("`{}` = ?", col.replace('`', "``")));
                where_bind.push(val);
            }
        }
        let sql = format!(
            "DELETE FROM `{}`.`{}` WHERE {} LIMIT 1",
            database.replace('`', "``"),
            table.replace('`', "``"),
            where_parts.join(" AND ")
        );
        let mut q = sqlx::query(&sql);
        for val in &where_bind {
            q = bind_mysql_value(q, val);
        }
        deleted += q
            .execute(&mut *conn)
            .await
            .map_err(RowmanceError::Database)?
            .rows_affected();
    }

    Ok((updated, inserted, deleted))
}

fn format_mysql_type(type_info: &sqlx::mysql::MySqlTypeInfo) -> String {
    let base = type_info.name().to_lowercase();
    let debug = format!("{type_info:?}");
    if let Some(size) = parse_max_size(&debug) {
        match base.as_str() {
            "varchar" | "char" | "varbinary" | "binary" | "bit" | "tinyint" => {
                return format!("{base}({size})");
            }
            _ => {}
        }
    }
    base
}

fn parse_max_size(debug: &str) -> Option<u32> {
    let prefix = "max_size: Some(";
    let start = debug.find(prefix)? + prefix.len();
    let end = debug[start..].find(')')? + start;
    debug[start..end].parse().ok()
}

fn mysql_value_to_json(row: &sqlx::mysql::MySqlRow, idx: usize) -> serde_json::Value {
    let col = &row.columns()[idx];
    let type_name = col.type_info().name().to_lowercase();
    let debug = format!("{:?}", col.type_info());
    let is_bool_col =
        type_name == "boolean" || (type_name == "tinyint" && parse_max_size(&debug) == Some(1));
    if is_bool_col {
        if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
            return v
                .map(serde_json::Value::Bool)
                .unwrap_or(serde_json::Value::Null);
        }
    }

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
    if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(idx) {
        return v
            .map(|dt| serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string()))
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
    if let Ok(v) = row.try_get::<Option<sqlx::mysql::types::MySqlTime>, _>(idx) {
        return v
            .map(|t| serde_json::Value::String(t.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<u16>, _>(idx) {
        return v
            .map(|n| serde_json::Value::from(n as i64))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(idx) {
        return v
            .map(|j| serde_json::Value::String(j.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    serde_json::Value::Null
}

fn bind_mysql_value<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    value: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
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
                        let before_ok =
                            i == 0 || (!bytes[i - 1].is_ascii_alphabetic() && bytes[i - 1] != b'_');
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
    use crate::connections::types::{DialectInfo, FkViolationHint};
    match db_type {
        "mysql" | "mariadb" => Some(DialectInfo {
            identifier_open: "`".into(),
            identifier_close: "`".into(),
            identifier_escape: "``".into(),
            uses_schema: true,
            db_label: "Database".into(),
            has_instance_databases: false,
            select_top: false,
            boolean_literals: false,
            uses_ilike: false,
            cast_to_text: "CAST({col} AS CHAR)".into(),
            supports_user_management: true,
            host_based_users: true,
            supports_roles: false,
            detects_sql_variables: true,
            warns_tx_database_mismatch: true,
            display_name: if db_type == "mariadb" {
                "MariaDB".into()
            } else {
                "MySQL".into()
            },
            default_column_type: "INT".into(),
            common_column_types: vec![
                "INT",
                "BIGINT",
                "SMALLINT",
                "TINYINT",
                "VARCHAR(255)",
                "TEXT",
                "LONGTEXT",
                "DATETIME",
                "DATE",
                "FLOAT",
                "DOUBLE",
                "DECIMAL(10,2)",
                "BOOLEAN",
                "JSON",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            supports_auto_increment: true,
            supports_column_comment: true,
            supports_change_column: true,
            supports_rename_column: false,
            uses_foreign_key_keyword: true,
            drop_index_syntax: "on_table".into(),
            default_new_column_type: "VARCHAR(255)".into(),
            uses_database_keyword: true,
            drop_schema_cascade: false,
            is_file_based: false,
            default_port: 3306,
            url_schemes: if db_type == "mariadb" {
                vec!["mariadb".into()]
            } else {
                vec!["mysql".into()]
            },
            url_template: "mysql://{username}:{password}@{host}:{port}/{database}".into(),
            fk_violation: Some(FkViolationHint {
                detect: "constraint fails".into(),
                table_pattern: r"constraint fails \(`[^`]*`\.`([^`]+)`".into(),
                column_pair_pattern: Some(
                    r"FOREIGN KEY \(`([^`]+)`\) REFERENCES `[^`]+` \(`([^`]+)`\)".into(),
                ),
                column_value_pattern: None,
            }),
            editor_dialect: "mysql".into(),
            explain_format: "mysql_json".into(),
            system_databases: vec![
                "information_schema".into(),
                "mysql".into(),
                "performance_schema".into(),
                "sys".into(),
            ],
            file_extensions: vec![],
        }),
        _ => None,
    }
}

// ── Pool adapter ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MySqlPoolAdapter {
    pub pool: sqlx::MySqlPool,
    pub read_only: bool,
}

#[async_trait]
impl crate::connections::engine::PoolAdapter for MySqlPoolAdapter {
    async fn disconnect(&self) {
        self.pool.close().await;
    }
    async fn ping(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
    fn get_engine(&self) -> std::sync::Arc<dyn crate::connections::engine::DatabaseEngine> {
        std::sync::Arc::new(MySqlEngine {
            pool: self.pool.clone(),
            read_only: self.read_only,
        })
    }
}

// ── Pool creation ─────────────────────────────────────────────────────────────

/// Reset MySQL connection to the given database using the text protocol (COM_QUERY).
async fn mysql_reset_db(conn: &mut sqlx::mysql::MySqlConnection, use_sql: &str) -> bool {
    use sqlx::Executor as _;
    conn.execute(sqlx::raw_sql(use_sql)).await.is_ok()
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
    use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
    use std::path::Path;

    let mut opts = MySqlConnectOptions::new()
        .host(host)
        .port(port)
        .database(database)
        .username(username);
    if !password.is_empty() {
        opts = opts.password(password);
    }

    if ssl_enabled {
        let ssl_mode = if ssl_ca_path.is_some() {
            MySqlSslMode::VerifyCa
        } else {
            MySqlSslMode::Required
        };
        opts = opts.ssl_mode(ssl_mode);

        if let Some(ca) = ssl_ca_path {
            opts = opts.ssl_ca(Path::new(ca));
        }
        if let Some(cert) = ssl_cert_path {
            if let Some(key) = ssl_key_path {
                opts = opts
                    .ssl_client_cert(Path::new(cert))
                    .ssl_client_key(Path::new(key));
            }
        }
    } else {
        opts = opts.ssl_mode(MySqlSslMode::Disabled);
    }

    let mut pool_opts = MySqlPoolOptions::new()
        .min_connections(0)
        .max_connections(pool_max)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .test_before_acquire(true);

    if read_only {
        pool_opts = pool_opts.after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET SESSION TRANSACTION READ ONLY")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        });
    }

    let db_esc = database.replace('`', "``");
    let use_sql: &'static str = Box::leak(format!("USE `{}`", db_esc).into_boxed_str());
    pool_opts = pool_opts.after_release(move |conn, _meta| {
        Box::pin(async move { Ok(mysql_reset_db(conn, use_sql).await) })
    });
    let p = pool_opts.connect_with(opts).await?;
    // Verify credentials and warm up one idle connection.
    p.acquire().await?;
    Ok(Box::new(MySqlPoolAdapter { pool: p, read_only }))
}

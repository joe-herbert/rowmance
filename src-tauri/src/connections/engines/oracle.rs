/// Oracle engine implementation of the `DatabaseEngine` trait.
/// Uses the `oracle` crate (sync ODPI-C bindings) with tokio::task::spawn_blocking.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::connections::engine::{DatabaseEngine, EngineTransaction};
use crate::connections::erd::group_into_tables;
use crate::connections::types::{
    BulkColumnRow, CapabilityStatus, ColumnInfo, ColumnMeta, EngineQueryResult, ErdColumn,
    ErdGraph, ErdRelation, ExplainResult, ForeignKeyInfo, IndexInfo, LockInfo, ProcessInfo,
    RowChange, RowDelete, ScheduledJob, ServerAdminCapabilityFlags, ServerStatus, ServerVariable,
    TableInfo, VarScope,
};
use crate::error::RowmanceError;

// ── Connection wrapper ────────────────────────────────────────────────────────

/// Wraps an oracle::Connection behind a raw pointer so we can declare it Send.
/// The oracle crate marks Connection as !Send (ODPI-C connections are not
/// thread-safe in the sense that they cannot be shared concurrently), but they
/// ARE safe to send to another thread when only one thread uses them at a time.
/// We enforce single-thread-at-a-time by extracting from the pool (or moving
/// out of OracleTransaction) before spawning the blocking task.
struct OracleConnWrapper(*mut oracle::Connection);

impl OracleConnWrapper {
    fn new(conn: oracle::Connection) -> Self {
        Self(Box::into_raw(Box::new(conn)))
    }

    fn conn(&self) -> &oracle::Connection {
        // SAFETY: pointer is valid for the lifetime of this wrapper.
        unsafe { &*self.0 }
    }
}

impl Drop for OracleConnWrapper {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: we own the pointer, created via Box::into_raw.
            unsafe { drop(Box::from_raw(self.0)) };
        }
    }
}

// SAFETY: ODPI-C connections are safe to send across threads when accessed by
// only one thread at a time. We guarantee this by using the pool/transaction
// design: a connection is always held by at most one thread at any moment.
unsafe impl Send for OracleConnWrapper {}
unsafe impl Sync for OracleConnWrapper {}

// ── Custom pool ───────────────────────────────────────────────────────────────

pub struct OraclePool {
    username: String,
    password: String,
    connect_string: String, // "//host:port/service"
    max_size: usize,
    conns: Mutex<Vec<OracleConnWrapper>>,
}

impl OraclePool {
    fn new(username: String, password: String, connect_string: String, max_size: usize) -> Self {
        Self {
            username,
            password,
            connect_string,
            max_size,
            conns: Mutex::new(Vec::new()),
        }
    }

    fn get(&self) -> Result<OracleConnWrapper, RowmanceError> {
        {
            let mut pool = self.conns.lock().unwrap();
            if let Some(conn) = pool.pop() {
                return Ok(conn);
            }
        }
        // Create a new connection
        let mut conn =
            oracle::Connection::connect(&self.username, &self.password, &self.connect_string)
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        // Disable auto-commit so we control transactions explicitly
        conn.set_autocommit(false);
        Ok(OracleConnWrapper::new(conn))
    }

    fn return_conn(&self, conn: OracleConnWrapper) {
        let mut pool = self.conns.lock().unwrap();
        if pool.len() < self.max_size {
            pool.push(conn);
        }
        // If at max_size, the connection is simply dropped/closed
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

pub struct OracleEngine {
    pub pool: Arc<OraclePool>,
    #[allow(dead_code)]
    pub read_only: bool,
    #[allow(dead_code)]
    pub initial_schema: String,
}

#[async_trait]
impl DatabaseEngine for OracleEngine {
    fn quote(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn placeholder(&self, n: usize) -> String {
        format!(":{}", n)
    }

    fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String {
        if limit == 0 {
            return sql.to_string();
        }
        let upper = sql.to_uppercase();
        if upper.contains("FETCH NEXT") || upper.contains("FETCH FIRST") {
            return sql.to_string();
        }
        format!("{sql} OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
    }

    fn parse_returns_rows(&self, sql: &str) -> bool {
        returns_rows_heuristic(sql)
    }

    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let sql = "
                SELECT DISTINCT username
                FROM all_users
                WHERE username NOT IN (
                    'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','XDB','WMSYS',
                    'CTXSYS','DVSYS','LBACSYS','MDSYS','ORDSYS','ORDDATA'
                )
                ORDER BY username
            ";
            let mut stmt = conn
                .conn()
                .statement(sql)
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
                let name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
                result.push(name);
            }
            pool.return_conn(conn);
            Ok(result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_tables(
        &self,
        database: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<TableInfo>, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = crate::connections::oracle::list_tables(conn.conn(), &owner);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_columns(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<ColumnInfo>, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = crate::connections::oracle::list_columns(conn.conn(), &owner, &table);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_all_columns(
        &self,
        database: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<BulkColumnRow>, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let pairs = crate::connections::oracle::list_all_columns(conn.conn(), &owner)?;
            pool.return_conn(conn);
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
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_indexes(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<IndexInfo>, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = crate::connections::oracle::list_indexes(conn.conn(), &owner, &table);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_foreign_keys(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = crate::connections::oracle::list_foreign_keys(conn.conn(), &owner, &table);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn count_table(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<i64, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = crate::connections::oracle::count_table(conn.conn(), &owner, &table);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn get_ddl(
        &self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
    ) -> Result<String, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = crate::connections::oracle::get_ddl(conn.conn(), &owner, &table);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn execute(
        &self,
        sql: &str,
        database: Option<&str>,
        _instance_db: Option<&str>,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        let pool = self.pool.clone();
        let sql = sql.to_string();
        let schema = database.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            if let Some(ref s) = schema {
                let alter_sql = format!(
                    "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                    s.replace('"', "\"\"")
                );
                let _ = conn.conn().execute(&alter_sql, &[]);
            }
            let result = execute_on_oracle_conn(conn.conn(), &sql, page_size, offset);
            pool.return_conn(conn);
            result
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn execute_ddl(&self, sql: &str) -> Result<(), RowmanceError> {
        let pool = self.pool.clone();
        let sql = sql.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            conn.conn()
                .execute(&sql, &[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            // DDL auto-commits in Oracle; no explicit commit needed but we do it for safety
            let _ = conn.conn().commit();
            pool.return_conn(conn);
            Ok(())
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn count_query_rows(
        &self,
        sql: &str,
        database: Option<&str>,
        _instance_db: Option<&str>,
    ) -> Option<i64> {
        let pool = self.pool.clone();
        let sql = sql.trim_end_matches(';').to_string();
        let schema = database.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Option<i64> {
            let conn = pool.get().ok()?;
            if let Some(ref s) = schema {
                let alter_sql = format!(
                    "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                    s.replace('"', "\"\"")
                );
                let _ = conn.conn().execute(&alter_sql, &[]);
            }
            let count_sql = format!("SELECT COUNT(*) FROM ({sql})");
            let result = conn.conn().query_row_as::<i64>(&count_sql, &[]).ok();
            pool.return_conn(conn);
            result
        })
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
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        let updates = updates.to_vec();
        let inserts = inserts.to_vec();
        let deletes = deletes.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result =
                apply_all_oracle(conn.conn(), &owner, &table, &updates, &inserts, &deletes);
            match result {
                Ok(counts) => {
                    conn.conn()
                        .commit()
                        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
                    pool.return_conn(conn);
                    Ok(counts)
                }
                Err(e) => {
                    let _ = conn.conn().rollback();
                    pool.return_conn(conn);
                    Err(e)
                }
            }
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn ping(&self) -> bool {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(_) => return false,
            };
            let ok = conn
                .conn()
                .query_row_as::<i64>("SELECT 1 FROM DUAL", &[])
                .is_ok();
            pool.return_conn(conn);
            ok
        })
        .await
        .unwrap_or(false)
    }

    async fn begin_transaction(
        &self,
        database: Option<&str>,
    ) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let pool = self.pool.clone();
        let schema = database.map(|s| s.to_string());
        tokio::task::spawn_blocking(
            move || -> Result<Box<dyn EngineTransaction>, RowmanceError> {
                let conn = pool.get()?;
                if let Some(ref s) = schema {
                    let alter_sql = format!(
                        "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                        s.replace('"', "\"\"")
                    );
                    let _ = conn.conn().execute(&alter_sql, &[]);
                }
                // Oracle starts transactions implicitly on first DML; no BEGIN needed.
                Ok(Box::new(OracleTransaction {
                    conn: Some(conn),
                    pool,
                }))
            },
        )
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn begin_session(&self) -> Result<Box<dyn EngineTransaction>, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(
            move || -> Result<Box<dyn EngineTransaction>, RowmanceError> {
                let conn = pool.get()?;
                Ok(Box::new(OracleTransaction {
                    conn: Some(conn),
                    pool,
                }))
            },
        )
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn explain(
        &self,
        sql: &str,
        database: Option<&str>,
        _instance_db: Option<&str>,
    ) -> Result<ExplainResult, RowmanceError> {
        let pool = self.pool.clone();
        let sql = sql.to_string();
        let schema = database.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            if let Some(ref s) = schema {
                let alter_sql = format!(
                    "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                    s.replace('"', "\"\"")
                );
                let _ = conn.conn().execute(&alter_sql, &[]);
            }
            let explain_sql = format!("EXPLAIN PLAN FOR {sql}");
            conn.conn()
                .execute(&explain_sql, &[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;

            let plan_sql =
                "SELECT plan_table_output FROM TABLE(DBMS_XPLAN.DISPLAY(NULL, NULL, 'ALL'))";
            let mut stmt = conn
                .conn()
                .statement(plan_sql)
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;

            let mut lines = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
                let line: String = row.get(0).unwrap_or_default();
                lines.push(line);
            }
            let plan_text = lines.join("\n");
            pool.return_conn(conn);

            let payload = serde_json::json!({ "text": plan_text });
            Ok(ExplainResult {
                raw_json: payload.to_string(),
                dialect: "oracle_text".to_string(),
            })
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn get_erd_graph(
        &self,
        database: &str,
        _instance_db: Option<&str>,
    ) -> Result<ErdGraph, RowmanceError> {
        let pool = self.pool.clone();
        let owner = database.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;

            let all_cols = crate::connections::oracle::list_all_columns(conn.conn(), &owner)?;
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
                let fks = crate::connections::oracle::list_foreign_keys(
                    conn.conn(),
                    &owner,
                    &table.name,
                )?;
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

            pool.return_conn(conn);
            Ok(ErdGraph {
                nodes,
                edges: all_fk_edges,
            })
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
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
        let pool = self.pool.clone();
        let owner = database.to_string();
        let table = table.to_string();
        let headers = headers.to_vec();
        let rows = rows.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let q = |ident: &str| format!("\"{}\"", ident.replace('"', "\"\""));

            if create_table {
                let col_defs: Vec<String> = headers
                    .iter()
                    .map(|h| format!("{} VARCHAR2(4000)", q(h)))
                    .collect();
                let ddl = format!(
                    "CREATE TABLE {}.{} ({})",
                    q(&owner),
                    q(&table),
                    col_defs.join(", ")
                );
                // Use execute and ignore "table already exists" error
                let _ = conn.conn().execute(&ddl, &[]);
                let _ = conn.conn().commit();
            }

            let col_list: Vec<String> = headers.iter().map(|h| q(h)).collect();
            let placeholders: Vec<String> = (1..=headers.len()).map(|i| format!(":{i}")).collect();
            let sql = format!(
                "INSERT INTO {}.{} ({}) VALUES ({})",
                q(&owner),
                q(&table),
                col_list.join(", "),
                placeholders.join(", ")
            );

            let mut inserted = 0u64;
            for row in &rows {
                let params: Vec<&dyn oracle::sql_type::ToSql> = row
                    .iter()
                    .map(|s| s as &dyn oracle::sql_type::ToSql)
                    .collect();
                conn.conn()
                    .execute(&sql, &params)
                    .map_err(|e| RowmanceError::Pool(e.to_string()))?;
                inserted += 1;
            }

            conn.conn()
                .commit()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            pool.return_conn(conn);
            Ok(inserted)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn probe_server_admin_capabilities(
        &self,
    ) -> Result<ServerAdminCapabilityFlags, RowmanceError> {
        let pool = self.pool.clone();
        let oracle_result = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let query_result = conn
                .conn()
                .statement("SELECT COUNT(*) FROM v$session WHERE rownum = 1")
                .build()
                .and_then(|mut s| s.query(&[]).map(|_| ()));
            pool.return_conn(conn);
            Ok::<_, RowmanceError>(query_result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))??;

        let status = match oracle_result {
            Ok(_) => CapabilityStatus::Supported,
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("insufficient privileges")
                    || msg.contains("ora-00942")
                    || msg.contains("ora-01031")
                {
                    CapabilityStatus::InsufficientPrivileges
                } else {
                    CapabilityStatus::NotSupported
                }
            }
        };

        Ok(ServerAdminCapabilityFlags {
            process_list: status,
            kill_session: CapabilityStatus::Supported,
            cancel_session: CapabilityStatus::NotSupported,
            server_status: CapabilityStatus::Supported,
            variables: CapabilityStatus::Supported,
            set_variable: CapabilityStatus::Supported,
            scheduled_jobs: CapabilityStatus::Supported,
            locks: CapabilityStatus::Supported,
            innodb_status: CapabilityStatus::NotSupported,
            vacuum_status: CapabilityStatus::NotSupported,
        })
    }

    async fn list_processes(&self) -> Result<Vec<ProcessInfo>, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let sql = "SELECT sid, serial#, username, machine, status, last_call_et, \
                              sql_text \
                       FROM v$session \
                       WHERE type = 'USER' \
                       ORDER BY last_call_et DESC";
            let mut stmt = conn
                .conn()
                .statement(sql)
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;

            let mut result = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
                let sid: i64 = row.get(0).unwrap_or(0);
                let serial: i64 = row.get(1).unwrap_or(0);
                let username: Option<String> = row.get(2).ok();
                let machine: Option<String> = row.get(3).ok();
                let status: Option<String> = row.get(4).ok();
                let last_call_et: Option<i64> = row.get(5).ok();
                let sql_text: Option<String> = row.get(6).ok();
                result.push(ProcessInfo {
                    id: format!("{sid},{serial}"),
                    user: username,
                    host: machine,
                    database: None,
                    command: status.clone(),
                    time_seconds: last_call_et.map(|v| v.max(0) as u64),
                    state: status,
                    info: sql_text,
                    can_kill: true,
                    can_cancel: false,
                });
            }
            pool.return_conn(conn);
            Ok(result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn kill_session(&self, session_id: &str) -> Result<(), RowmanceError> {
        let pool = self.pool.clone();
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            if !crate::commands::server_admin::is_valid_oracle_session_id(&session_id) {
                return Err(RowmanceError::ConnectionNotFound(
                    "Invalid session id".to_string(),
                ));
            }
            let sql = format!("ALTER SYSTEM KILL SESSION '{session_id}' IMMEDIATE");
            conn.conn()
                .execute(&sql, &[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            pool.return_conn(conn);
            Ok(())
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn get_server_status(&self) -> Result<ServerStatus, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;

            let mut stmt = conn
                .conn()
                .statement("SELECT banner FROM v$version WHERE banner LIKE 'Oracle%'")
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let version = rows
                .into_iter()
                .next()
                .and_then(|r| r.ok())
                .and_then(|r| {
                    let v: Option<String> = r.get(0).ok();
                    v
                })
                .unwrap_or_else(|| "Oracle".to_string());

            let mut uptime_stmt = conn
                .conn()
                .statement("SELECT (SYSDATE - startup_time) * 86400 AS uptime_secs FROM v$instance")
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let uptime_rows = uptime_stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let uptime_seconds = uptime_rows
                .into_iter()
                .next()
                .and_then(|r| r.ok())
                .and_then(|r| {
                    let v: Option<f64> = r.get(0).ok();
                    v
                })
                .map(|v| v.max(0.0) as u64)
                .unwrap_or(0);

            let mut sess_stmt = conn
                .conn()
                .statement("SELECT COUNT(*) FROM v$session WHERE type = 'USER'")
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let sess_rows = sess_stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let connections_current = sess_rows
                .into_iter()
                .next()
                .and_then(|r| r.ok())
                .and_then(|r| {
                    let v: Option<i64> = r.get(0).ok();
                    v
                })
                .map(|v| v.max(0) as u64)
                .unwrap_or(0);

            pool.return_conn(conn);
            Ok(ServerStatus {
                version,
                uptime_seconds,
                connections_current,
                connections_max: None,
                queries_per_second: None,
                cache_hit_ratio: None,
                extra: std::collections::HashMap::new(),
            })
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_variables(&self) -> Result<Vec<ServerVariable>, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn
                .conn()
                .statement(
                    "SELECT name, value, description, isses_modifiable, issys_modifiable \
                 FROM v$parameter \
                 ORDER BY name",
                )
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;

            let mut vars = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
                let name: String = row.get(0).unwrap_or_default();
                let value: String = row.get(1).unwrap_or_default();
                let desc: Option<String> = row.get(2).ok();
                let ses_mod: String = row.get(3).unwrap_or_default();
                let sys_mod: String = row.get(4).unwrap_or_default();

                let is_dynamic =
                    ses_mod == "IMMEDIATE" || sys_mod == "IMMEDIATE" || sys_mod == "DEFERRED";
                let restart_required = sys_mod == "FALSE" && ses_mod == "FALSE";

                vars.push(ServerVariable {
                    name,
                    value,
                    scope: VarScope::Global,
                    is_dynamic,
                    restart_required,
                    description: desc,
                    data_type: None,
                });
            }
            pool.return_conn(conn);
            Ok(vars)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn set_variable(
        &self,
        name: &str,
        value: &str,
        _scope: VarScope,
    ) -> Result<(), RowmanceError> {
        let pool = self.pool.clone();
        let name = name.to_string();
        let value = value.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(RowmanceError::ConnectionNotFound(
                    "Invalid parameter name".to_string(),
                ));
            }
            let ev = value.replace('\'', "''");
            let sql = format!("ALTER SYSTEM SET {name} = '{ev}' SCOPE=BOTH");
            conn.conn()
                .execute(&sql, &[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            pool.return_conn(conn);
            Ok(())
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_locks(&self) -> Result<Vec<LockInfo>, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn
                .conn()
                .statement(
                    "SELECT l.sid, l.type, l.lmode, l.request, o.object_name, \
                        l.block, l.id1, l.id2 \
                 FROM v$lock l \
                 LEFT JOIN dba_objects o ON o.object_id = l.id1 \
                 WHERE l.block > 0 OR l.request > 0 \
                 ORDER BY l.block DESC, l.sid",
                )
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;

            let mut locks = Vec::new();
            for (i, row_result) in rows.into_iter().enumerate() {
                let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
                let sid: i64 = row.get(0).unwrap_or(0);
                let lock_type: String = row.get(1).unwrap_or_default();
                let lmode: i64 = row.get(2).unwrap_or(0);
                let request: i64 = row.get(3).unwrap_or(0);
                let object_name: Option<String> = row.get(4).ok();
                let block: i64 = row.get(5).unwrap_or(0);

                locks.push(LockInfo {
                    lock_id: format!("lock-{i}"),
                    blocker_session_id: if block > 0 {
                        Some(sid.to_string())
                    } else {
                        None
                    },
                    waiting_session_id: if request > 0 {
                        Some(sid.to_string())
                    } else {
                        None
                    },
                    lock_type,
                    lock_mode: lmode.to_string(),
                    object_name,
                    duration_ms: None,
                });
            }
            pool.return_conn(conn);
            Ok(locks)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }

    async fn list_scheduled_jobs(&self) -> Result<Vec<ScheduledJob>, RowmanceError> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn
                .conn()
                .statement(
                    "SELECT job_name, enabled, last_start_date, next_run_date, \
                        repeat_interval \
                 FROM dba_scheduler_jobs \
                 ORDER BY job_name",
                )
                .build()
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let rows = stmt
                .query(&[])
                .map_err(|e| RowmanceError::Pool(e.to_string()))?;

            let mut jobs = Vec::new();
            for row_result in rows {
                let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
                let name: String = row.get(0).unwrap_or_default();
                let enabled: String = row.get(1).unwrap_or_default();
                let last_run: Option<String> = row.get(2).ok().and_then(|v: Option<String>| v);
                let next_run: Option<String> = row.get(3).ok().and_then(|v: Option<String>| v);
                let schedule: String = row.get(4).unwrap_or_else(|_| "ONE TIME".to_string());
                jobs.push(ScheduledJob {
                    id: name.clone(),
                    name,
                    schedule,
                    enabled: enabled == "TRUE",
                    last_run,
                    next_run,
                    body: None,
                });
            }
            pool.return_conn(conn);
            Ok(jobs)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    }
}

// ── OracleTransaction ─────────────────────────────────────────────────────────

pub struct OracleTransaction {
    conn: Option<OracleConnWrapper>,
    pool: Arc<OraclePool>,
}

#[async_trait]
impl EngineTransaction for OracleTransaction {
    async fn execute(
        &mut self,
        sql: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError> {
        let conn = self.conn.take().ok_or_else(|| {
            RowmanceError::Pool("Transaction connection already consumed".to_string())
        })?;
        let sql = sql.to_string();
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let result = execute_on_oracle_conn(conn.conn(), &sql, page_size, offset);
            (conn, result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        self.conn = Some(conn);
        result
    }

    async fn apply_changes(
        &mut self,
        database: &str,
        table: &str,
        _instance_db: Option<&str>,
        updates: &[RowChange],
        inserts: &[HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError> {
        let conn = self.conn.take().ok_or_else(|| {
            RowmanceError::Pool("Transaction connection already consumed".to_string())
        })?;
        let owner = database.to_string();
        let table = table.to_string();
        let updates = updates.to_vec();
        let inserts = inserts.to_vec();
        let deletes = deletes.to_vec();

        let (conn, result) = tokio::task::spawn_blocking(move || {
            let result =
                apply_all_oracle(conn.conn(), &owner, &table, &updates, &inserts, &deletes);
            (conn, result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        self.conn = Some(conn);
        result
    }

    async fn execute_raw(&mut self, sql: &str) -> Result<(), RowmanceError> {
        let conn = self.conn.take().ok_or_else(|| {
            RowmanceError::Pool("Transaction connection already consumed".to_string())
        })?;
        let sql = sql.to_string();
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let result = conn
                .conn()
                .execute(&sql, &[])
                .map(|_| ())
                .map_err(|e| RowmanceError::Pool(e.to_string()));
            (conn, result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        self.conn = Some(conn);
        result
    }

    async fn commit(&mut self) -> Result<(), RowmanceError> {
        let conn = self.conn.take().ok_or_else(|| {
            RowmanceError::Pool("Transaction connection already consumed".to_string())
        })?;
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let result = conn
                .conn()
                .commit()
                .map_err(|e| RowmanceError::Pool(e.to_string()));
            (conn, result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        self.pool.return_conn(conn);
        result
    }

    async fn rollback(&mut self) -> Result<(), RowmanceError> {
        let conn = self.conn.take().ok_or_else(|| {
            RowmanceError::Pool("Transaction connection already consumed".to_string())
        })?;
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let result = conn
                .conn()
                .rollback()
                .map_err(|e| RowmanceError::Pool(e.to_string()));
            (conn, result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        self.pool.return_conn(conn);
        result
    }

    async fn switch_context(&mut self, database: &str) -> Result<(), RowmanceError> {
        let conn = self.conn.take().ok_or_else(|| {
            RowmanceError::Pool("Transaction connection already consumed".to_string())
        })?;
        let schema = database.to_string();
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let alter_sql = format!(
                "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                schema.replace('"', "\"\"")
            );
            let result = conn
                .conn()
                .execute(&alter_sql, &[])
                .map(|_| ())
                .map_err(|e| RowmanceError::Pool(e.to_string()));
            (conn, result)
        })
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        self.conn = Some(conn);
        result
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn execute_on_oracle_conn(
    conn: &oracle::Connection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> Result<EngineQueryResult, RowmanceError> {
    let exec_sql = sql.trim_end_matches(';');
    let is_select = returns_rows_heuristic(exec_sql);

    if is_select {
        let paginated = if page_size > 0 {
            let upper = exec_sql.to_uppercase();
            if upper.contains("FETCH NEXT") || upper.contains("FETCH FIRST") {
                exec_sql.to_string()
            } else {
                format!("{exec_sql} OFFSET {offset} ROWS FETCH NEXT {page_size} ROWS ONLY")
            }
        } else {
            exec_sql.to_string()
        };

        let mut stmt = conn
            .statement(&paginated)
            .build()
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let rows = stmt
            .query(&[])
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;

        let col_info = rows.column_info();
        let columns: Vec<ColumnMeta> = col_info
            .iter()
            .map(|c| ColumnMeta {
                name: c.name().to_string(),
                data_type: format!("{:?}", c.oracle_type()),
                nullable: true,
                is_primary_key: false,
                is_foreign_key: false,
            })
            .collect();

        let mut data: Vec<Vec<serde_json::Value>> = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
            let row_vals: Vec<serde_json::Value> = (0..columns.len())
                .map(|i| crate::connections::oracle::oracle_value_to_json(&row, i))
                .collect();
            data.push(row_vals);
        }

        Ok(EngineQueryResult {
            columns,
            rows: data,
            affected_rows: None,
        })
    } else {
        let stmt = conn
            .execute(exec_sql, &[])
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let affected = stmt.row_count().unwrap_or(0);
        Ok(EngineQueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: Some(affected),
        })
    }
}

fn apply_all_oracle(
    conn: &oracle::Connection,
    schema: &str,
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
                let s = format!("{} = :{}", q(col), param_idx);
                param_idx += 1;
                s
            })
            .collect();

        let where_pairs: Vec<(&String, &serde_json::Value)> = change.primary_keys.iter().collect();
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_vals: Vec<Option<String>> = Vec::new();
        for (col, val) in &where_pairs {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = :{}", q(col), param_idx));
                param_idx += 1;
                where_vals.push(crate::connections::oracle::json_value_to_oracle_string(val));
            }
        }

        let sql = format!(
            "UPDATE {}.{} SET {} WHERE {}",
            q(schema),
            q(table),
            set_clause.join(", "),
            where_parts.join(" AND ")
        );

        let mut all_vals: Vec<Option<String>> = change
            .changes
            .values()
            .map(crate::connections::oracle::json_value_to_oracle_string)
            .collect();
        all_vals.extend(where_vals);

        let params: Vec<&dyn oracle::sql_type::ToSql> = all_vals
            .iter()
            .map(|v| v as &dyn oracle::sql_type::ToSql)
            .collect();

        let stmt = conn
            .execute(&sql, &params)
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        updated += stmt.row_count().unwrap_or(0);
    }

    // Inserts
    for values in inserts {
        if values.is_empty() {
            continue;
        }
        let cols: Vec<(&String, &serde_json::Value)> = values.iter().collect();
        let col_list: Vec<String> = cols.iter().map(|(c, _)| q(c)).collect();
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!(":{i}")).collect();
        let sql = format!(
            "INSERT INTO {}.{} ({}) VALUES ({})",
            q(schema),
            q(table),
            col_list.join(", "),
            placeholders.join(", ")
        );
        let vals: Vec<Option<String>> = cols
            .iter()
            .map(|(_, v)| crate::connections::oracle::json_value_to_oracle_string(v))
            .collect();
        let params: Vec<&dyn oracle::sql_type::ToSql> = vals
            .iter()
            .map(|v| v as &dyn oracle::sql_type::ToSql)
            .collect();
        conn.execute(&sql, &params)
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        inserted += 1;
    }

    // Deletes
    for row_del in deletes {
        if row_del.primary_keys.is_empty() {
            continue;
        }
        let mut param_idx = 1usize;
        let mut where_parts: Vec<String> = Vec::new();
        let mut where_vals: Vec<Option<String>> = Vec::new();
        for (col, val) in &row_del.primary_keys {
            if val.is_null() {
                where_parts.push(format!("{} IS NULL", q(col)));
            } else {
                where_parts.push(format!("{} = :{}", q(col), param_idx));
                param_idx += 1;
                where_vals.push(crate::connections::oracle::json_value_to_oracle_string(val));
            }
        }
        let sql = format!(
            "DELETE FROM {}.{} WHERE {}",
            q(schema),
            q(table),
            where_parts.join(" AND ")
        );
        let params: Vec<&dyn oracle::sql_type::ToSql> = where_vals
            .iter()
            .map(|v| v as &dyn oracle::sql_type::ToSql)
            .collect();
        let stmt = conn
            .execute(&sql, &params)
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        deleted += stmt.row_count().unwrap_or(0);
    }

    Ok((updated, inserted, deleted))
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
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("DESC ")
}

// ── Dialect info ──────────────────────────────────────────────────────────────

pub fn dialect_info(db_type: &str) -> Option<crate::connections::types::DialectInfo> {
    use crate::connections::types::DialectInfo;
    match db_type {
        "oracle" => Some(DialectInfo {
            identifier_open: "\"".into(),
            identifier_close: "\"".into(),
            identifier_escape: "\"\"".into(),
            uses_schema: true,
            db_label: "Schema".into(),
            has_instance_databases: false,
            requires_database: true,
            select_top: false,
            boolean_literals: false,
            uses_ilike: false,
            cast_to_text: "CAST({col} AS VARCHAR2(4000))".into(),
            supports_user_management: false,
            host_based_users: false,
            supports_roles: false,
            detects_sql_variables: false,
            warns_tx_database_mismatch: false,
            display_name: "Oracle".into(),
            default_column_type: "NUMBER GENERATED ALWAYS AS IDENTITY".into(),
            common_column_types: vec![
                "NUMBER",
                "VARCHAR2(255)",
                "VARCHAR2(4000)",
                "NVARCHAR2(255)",
                "CHAR(1)",
                "DATE",
                "TIMESTAMP",
                "CLOB",
                "BLOB",
                "FLOAT",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            supports_auto_increment: false,
            supports_column_comment: false,
            supports_change_column: false,
            supports_rename_column: true,
            uses_foreign_key_keyword: false,
            drop_index_syntax: "simple".into(),
            default_new_column_type: "VARCHAR2(255)".into(),
            uses_database_keyword: false,
            drop_schema_cascade: true,
            is_file_based: false,
            default_port: 1521,
            url_schemes: vec!["oracle".into()],
            url_template: "oracle://{username}:{password}@{host}:{port}/{database}".into(),
            fk_violation: None,
            editor_dialect: "sql".into(),
            explain_format: "oracle_text".into(),
            system_databases: vec![
                "SYS".into(),
                "SYSTEM".into(),
                "OUTLN".into(),
                "DBSNMP".into(),
                "APPQOSSYS".into(),
                "XDB".into(),
                "WMSYS".into(),
                "CTXSYS".into(),
                "DVSYS".into(),
                "LBACSYS".into(),
                "MDSYS".into(),
                "ORDSYS".into(),
                "ORDDATA".into(),
            ],
            file_extensions: vec![],
        }),
        _ => None,
    }
}

// ── Pool adapter ──────────────────────────────────────────────────────────────

pub struct OraclePoolAdapter {
    pub pool: Arc<OraclePool>,
    pub read_only: bool,
    pub initial_schema: String,
}

impl std::fmt::Debug for OraclePoolAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OraclePoolAdapter").finish()
    }
}

#[async_trait]
impl crate::connections::engine::PoolAdapter for OraclePoolAdapter {
    async fn disconnect(&self) {
        // Connections are dropped from the pool when the Arc is dropped.
        // Drain the pool so existing connections close gracefully.
        if let Ok(mut pool) = self.pool.conns.lock() {
            pool.clear();
        }
    }

    async fn ping(&self) -> bool {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(_) => return false,
            };
            let ok = conn
                .conn()
                .query_row_as::<i64>("SELECT 1 FROM DUAL", &[])
                .is_ok();
            pool.return_conn(conn);
            ok
        })
        .await
        .unwrap_or(false)
    }

    fn get_engine(&self) -> Arc<dyn crate::connections::engine::DatabaseEngine> {
        Arc::new(OracleEngine {
            pool: self.pool.clone(),
            read_only: self.read_only,
            initial_schema: self.initial_schema.clone(),
        })
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
    _ssl_enabled: bool,
    _ssl_ca_path: Option<&str>,
    read_only: bool,
) -> Result<Box<dyn crate::connections::engine::PoolAdapter>, crate::error::RowmanceError> {
    let connect_string = format!("//{}:{}/{}", host, port, database);
    let username = username.to_string();
    let password = password.to_string();
    let connect_string_clone = connect_string.clone();
    let username_clone = username.clone();
    let password_clone = password.clone();

    // Verify credentials before building the pool
    tokio::task::spawn_blocking(move || {
        oracle::Connection::connect(&username_clone, &password_clone, &connect_string_clone)
            .map_err(|e| crate::error::RowmanceError::Pool(e.to_string()))
    })
    .await
    .map_err(|e| crate::error::RowmanceError::Pool(e.to_string()))??;

    let pool = Arc::new(OraclePool::new(
        username,
        password,
        connect_string,
        pool_max as usize,
    ));

    Ok(Box::new(OraclePoolAdapter {
        pool,
        read_only,
        initial_schema: database.to_string(),
    }))
}

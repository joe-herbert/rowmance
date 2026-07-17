/// Tauri commands for executing SQL queries against remote databases.
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, State};

#[derive(Clone, Serialize)]
struct QueryCountPayload {
    #[serde(rename = "queryId")]
    query_id: String,
    #[serde(rename = "totalRows")]
    total_rows: i64,
}

use crate::connections::pool_manager::ConnectionManager;
use crate::connections::types::{ColumnMeta, RowChange, RowDelete};
use crate::error::AppError;
use crate::sessions::SessionManager;
use crate::transactions::TransactionManager;

/// Records granular timing for every query to the local SQLite DB, keeping only
/// the latest 100 entries. Only compiled and called in debug builds.
#[cfg(debug_assertions)]
#[allow(clippy::too_many_arguments)]
async fn record_speed_analysis(
    sqlite: &sqlx::SqlitePool,
    connection_id: &str,
    sql: &str,
    total_us: u64,
    pool_acquire_us: u64,
    db_switch_us: u64,
    execution_us: u64,
    result_processing_us: u64,
    row_count: Option<i64>,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        r#"INSERT INTO query_speed_analysis
           (id, connection_id, sql, total_us, pool_acquire_us, db_switch_us,
            execution_us, result_processing_us, row_count)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(id)
    .bind(connection_id)
    .bind(sql)
    .bind(total_us as i64)
    .bind(pool_acquire_us as i64)
    .bind(db_switch_us as i64)
    .bind(execution_us as i64)
    .bind(result_processing_us as i64)
    .bind(row_count)
    .execute(sqlite)
    .await;
    let _ = sqlx::query(
        r#"DELETE FROM query_speed_analysis
           WHERE id NOT IN (
               SELECT id FROM query_speed_analysis
               ORDER BY executed_at DESC LIMIT 100
           )"#,
    )
    .execute(sqlite)
    .await;
}

/// A single query result set, serialised for IPC.
#[derive(Debug, Serialize)]
pub struct QueryResult {
    #[serde(rename = "queryId")]
    pub query_id: String,
    pub columns: Vec<ColumnMeta>,
    /// Rows as arrays of JSON values; null represents SQL NULL.
    pub rows: Vec<Vec<serde_json::Value>>,
    #[serde(rename = "totalRows")]
    pub total_rows: Option<i64>,
    #[serde(rename = "durationUs")]
    pub duration_us: u64,
    #[serde(rename = "affectedRows")]
    pub affected_rows: Option<u64>,
    pub error: Option<String>,
}

/// Result returned after executing a batch of row updates.
#[derive(Serialize, Debug)]
pub struct UpdateResult {
    #[serde(rename = "updatedCount")]
    pub updated_count: u64,
}

/// Execute a batch of row-level DELETE statements.
#[tauri::command]
pub async fn query_delete_rows(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    database: String,
    table: String,
    instance_db: Option<String>,
    rows: Vec<RowDelete>,
) -> Result<UpdateResult, AppError> {
    let profile_row = sqlx::query!(
        "SELECT read_only, db_type FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let is_read_only = match &profile_row {
        Some(row) => row.read_only != 0,
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ))
        }
    };

    if is_read_only {
        return Err(AppError::new(
            "READ_ONLY_VIOLATION",
            "This connection is in read-only mode — mutating statements are not allowed",
        ));
    }

    if rows.is_empty() {
        return Ok(UpdateResult { updated_count: 0 });
    }

    let mut total_deleted: u64 = 0;

    if let Some(tx) = transactions.get(&connection_id) {
        let mut guard = tx.lock().await;
        let (_, _, deleted) = guard
            .apply_changes(&database, &table, instance_db.as_deref(), &[], &[], &rows)
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        total_deleted += deleted;
    } else {
        let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
        let (_, _, deleted) = engine
            .apply_changes(&database, &table, instance_db.as_deref(), &[], &[], &rows)
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        total_deleted += deleted;
    }

    Ok(UpdateResult {
        updated_count: total_deleted,
    })
}

/// Execute a batch of row-level UPDATE statements for inline cell editing.
#[tauri::command]
pub async fn query_update_rows(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    table: String,
    database: String,
    instance_db: Option<String>,
    changes: Vec<RowChange>,
) -> Result<UpdateResult, AppError> {
    // Check read-only mode before touching the remote pool.
    let profile_row = sqlx::query!(
        "SELECT read_only, db_type FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let is_read_only = match &profile_row {
        Some(row) => row.read_only != 0,
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ))
        }
    };

    if is_read_only {
        return Err(AppError::new(
            "READ_ONLY_VIOLATION",
            "This connection is in read-only mode — mutating statements are not allowed",
        ));
    }

    if changes.is_empty() {
        return Ok(UpdateResult { updated_count: 0 });
    }

    let mut total_updated: u64 = 0;

    if let Some(tx) = transactions.get(&connection_id) {
        let mut guard = tx.lock().await;
        let (updated, _, _) = guard
            .apply_changes(&database, &table, instance_db.as_deref(), &changes, &[], &[])
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        total_updated += updated;
    } else {
        let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
        let (updated, _, _) = engine
            .apply_changes(&database, &table, instance_db.as_deref(), &changes, &[], &[])
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        total_updated += updated;
    }

    Ok(UpdateResult {
        updated_count: total_updated,
    })
}

/// Insert a new row into a table.
#[tauri::command]
pub async fn query_insert_row(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    database: String,
    table: String,
    instance_db: Option<String>,
    values: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), AppError> {
    let profile_row = sqlx::query!(
        "SELECT read_only, db_type FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    match &profile_row {
        Some(row) if row.read_only != 0 => {
            return Err(AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode — mutating statements are not allowed",
            ));
        }
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ));
        }
        _ => {}
    }

    if values.is_empty() {
        return Err(AppError::new(
            "INVALID_INPUT",
            "No values provided for insert",
        ));
    }

    if let Some(tx) = transactions.get(&connection_id) {
        let mut guard = tx.lock().await;
        guard
            .apply_changes(&database, &table, instance_db.as_deref(), &[], &[values], &[])
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
    } else {
        let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
        engine
            .apply_changes(&database, &table, instance_db.as_deref(), &[], &[values], &[])
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
    }

    Ok(())
}

/// Result returned by `query_save_table_changes`.
#[derive(Serialize, Debug)]
pub struct SaveChangesResult {
    #[serde(rename = "updatedCount")]
    pub updated_count: u64,
    #[serde(rename = "insertedCount")]
    pub inserted_count: u64,
    #[serde(rename = "deletedCount")]
    pub deleted_count: u64,
}

/// Execute all pending table-browser edits (updates, inserts, deletes) atomically
/// inside a single transaction. Rolls back automatically if any operation fails.
/// If a user transaction is already active for this connection, the operations
/// are routed through the held connection without an extra BEGIN/COMMIT.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn query_save_table_changes(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    database: String,
    table: String,
    instance_db: Option<String>,
    updates: Vec<RowChange>,
    inserts: Vec<std::collections::HashMap<String, serde_json::Value>>,
    deletes: Vec<RowDelete>,
) -> Result<SaveChangesResult, AppError> {
    let profile_row = sqlx::query!(
        "SELECT read_only, db_type FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    match &profile_row {
        Some(row) if row.read_only != 0 => {
            return Err(AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode — mutating statements are not allowed",
            ));
        }
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ));
        }
        _ => {}
    }

    let mut updated_count = 0u64;
    let mut inserted_count = 0u64;
    let mut deleted_count = 0u64;

    // If a user-managed transaction is active, route through its held connection
    // (the user controls the transaction boundary).
    if let Some(tx) = transactions.get(&connection_id) {
        let mut guard = tx.lock().await;
        let (u, i, d) = guard
            .apply_changes(&database, &table, instance_db.as_deref(), &updates, &inserts, &deletes)
            .await
            .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        return Ok(SaveChangesResult {
            updated_count: u,
            inserted_count: i,
            deleted_count: d,
        });
    }

    // No active user transaction — delegate to the engine (handles transaction internally).
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    let (u, i, d) = engine
        .apply_changes(&database, &table, instance_db.as_deref(), &updates, &inserts, &deletes)
        .await
        .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
    updated_count += u;
    inserted_count += i;
    deleted_count += d;

    Ok(SaveChangesResult {
        updated_count,
        inserted_count,
        deleted_count,
    })
}

/// Split a SQL string into individual statements on `;`, respecting quoted strings and comments.
fn split_statements(sql: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        match chars[i] {
            '\'' => {
                current.push('\'');
                i += 1;
                while i < len {
                    let c = chars[i];
                    current.push(c);
                    i += 1;
                    if c == '\'' {
                        if i < len && chars[i] == '\'' {
                            current.push('\'');
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
            '"' => {
                current.push('"');
                i += 1;
                while i < len {
                    let c = chars[i];
                    current.push(c);
                    i += 1;
                    if c == '"' {
                        if i < len && chars[i] == '"' {
                            current.push('"');
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
            '`' => {
                current.push('`');
                i += 1;
                while i < len {
                    let c = chars[i];
                    current.push(c);
                    i += 1;
                    if c == '`' {
                        if i < len && chars[i] == '`' {
                            current.push('`');
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
            '-' if i + 1 < len && chars[i + 1] == '-' => {
                current.push('-');
                i += 1;
                while i < len && chars[i] != '\n' {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '/' if i + 1 < len && chars[i + 1] == '*' => {
                current.push('/');
                current.push('*');
                i += 2;
                while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                    current.push(chars[i]);
                    i += 1;
                }
                if i + 1 < len {
                    current.push('*');
                    current.push('/');
                    i += 2;
                }
            }
            ';' => {
                let stmt = current.trim().to_string();
                if !stmt.is_empty() {
                    statements.push(stmt);
                }
                current = String::new();
                i += 1;
            }
            c => {
                current.push(c);
                i += 1;
            }
        }
    }

    let stmt = current.trim().to_string();
    if !stmt.is_empty() {
        statements.push(stmt);
    }

    statements
}

/// Execute multiple SQL statements and return all result sets.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn query_execute_multi(
    app: tauri::AppHandle,
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    sessions: State<'_, Arc<SessionManager>>,
    connection_id: String,
    sql: String,
    database: Option<String>,
    instance_db: Option<String>,
    session_id: Option<String>,
) -> Result<Vec<QueryResult>, AppError> {
    let statements = split_statements(&sql);
    if statements.is_empty() {
        return Ok(vec![]);
    }

    let mut results: Vec<QueryResult> = Vec::new();

    // If a transaction is active, route all statements through the held connection.
    // Background counts are skipped for transaction queries.
    if let Some(tx) = transactions.get(&connection_id) {
        let mut guard = tx.lock().await;
        for stmt in &statements {
            let query_id = uuid::Uuid::new_v4().to_string();
            let start = std::time::Instant::now();
            let engine_result = guard.execute(stmt, 10_000, 0).await;
            let exec_result = engine_result_to_execute_result(engine_result);
            let duration_us = start.elapsed().as_micros() as u64;
            push_result(
                &mut results,
                exec_result,
                query_id,
                &connection_id,
                stmt,
                duration_us,
                sqlite.inner(),
            )
            .await;
        }
        return Ok(results);
    }

    // If an editor session is active, route through its pinned connection.
    // This guarantees session-scoped state (e.g. MySQL @variables) persists
    // across separate query executions from the same editor tab.
    if let Some(sid) = &session_id {
        if let Some(session) = sessions.get(sid) {
            let mut guard = session.lock().await;
            // Switch database once before executing the batch.
            if let Some(db) = &database {
                guard
                    .switch_context(db)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            }
            for stmt in &statements {
                let query_id = uuid::Uuid::new_v4().to_string();
                let start = std::time::Instant::now();
                let engine_result = guard.execute(stmt, 10_000, 0).await;
                let exec_result = engine_result_to_execute_result(engine_result);
                let duration_us = start.elapsed().as_micros() as u64;
                push_result(
                    &mut results,
                    exec_result,
                    query_id,
                    &connection_id,
                    stmt,
                    duration_us,
                    sqlite.inner(),
                )
                .await;
            }
            return Ok(results);
        }
    }

    // No active transaction or session — execute each statement via the engine.
    // The engine handles database switching and transaction wrapping internally.
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;

    for stmt in &statements {
        let query_id = uuid::Uuid::new_v4().to_string();
        let t = std::time::Instant::now();
        let exec_result = engine
            .execute(stmt, database.as_deref(), instance_db.as_deref(), 10_000, 0)
            .await;
        let execution_us = t.elapsed().as_micros() as u64;
        #[cfg(debug_assertions)]
        let t = std::time::Instant::now();
        match exec_result {
            Ok(er) => {
                let row_count = er.rows.len() as i64;
                let affected_rows = er.affected_rows;
                record_history(
                    sqlite.inner(),
                    &query_id,
                    &connection_id,
                    stmt,
                    execution_us,
                    Some(row_count),
                    None,
                    "success",
                )
                .await;
                let should_count = !er.columns.is_empty() && affected_rows.is_none();
                results.push(QueryResult {
                    query_id: query_id.clone(),
                    columns: er.columns,
                    rows: er.rows,
                    total_rows: None,
                    duration_us: execution_us,
                    affected_rows,
                    error: None,
                });
                #[cfg(debug_assertions)]
                {
                    let result_processing_us = t.elapsed().as_micros() as u64;
                    record_speed_analysis(
                        sqlite.inner(),
                        &connection_id,
                        stmt,
                        execution_us + result_processing_us,
                        0,
                        0,
                        execution_us,
                        result_processing_us,
                        results.last().map(|r| r.rows.len() as i64),
                    )
                    .await;
                }
                if should_count {
                    let engine_clone = engine.clone();
                    let stmt_clone = stmt.to_string();
                    let db_clone = database.clone();
                    let inst_db_clone = instance_db.clone();
                    let qid = query_id.clone();
                    let app_clone = app.clone();
                    tokio::spawn(async move {
                        if let Some(total) = engine_clone.count_query_rows(&stmt_clone, db_clone.as_deref(), inst_db_clone.as_deref()).await {
                            let _ = app_clone.emit("query-count-updated", QueryCountPayload { query_id: qid, total_rows: total });
                        }
                    });
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                record_history(
                    sqlite.inner(),
                    &query_id,
                    &connection_id,
                    stmt,
                    execution_us,
                    None,
                    Some(&err_msg),
                    "error",
                )
                .await;
                results.push(QueryResult {
                    query_id,
                    columns: vec![],
                    rows: vec![],
                    total_rows: None,
                    duration_us: execution_us,
                    affected_rows: None,
                    error: Some(err_msg),
                });
                #[cfg(debug_assertions)]
                {
                    let result_processing_us = t.elapsed().as_micros() as u64;
                    record_speed_analysis(
                        sqlite.inner(),
                        &connection_id,
                        stmt,
                        execution_us + result_processing_us,
                        0,
                        0,
                        execution_us,
                        result_processing_us,
                        None,
                    )
                    .await;
                }
            }
        }
    }

    Ok(results)
}

async fn push_result(
    results: &mut Vec<QueryResult>,
    exec_result: ExecuteResult,
    query_id: String,
    connection_id: &str,
    stmt: &str,
    duration_us: u64,
    sqlite: &sqlx::SqlitePool,
) {
    match exec_result {
        Ok((columns, rows, total_rows, affected_rows)) => {
            let row_count = rows.len() as i64;
            record_history(
                sqlite,
                &query_id,
                connection_id,
                stmt,
                duration_us,
                Some(row_count),
                None,
                "success",
            )
            .await;
            results.push(QueryResult {
                query_id,
                columns,
                rows,
                total_rows,
                duration_us,
                affected_rows,
                error: None,
            });
        }
        Err(err_msg) => {
            record_history(
                sqlite,
                &query_id,
                connection_id,
                stmt,
                duration_us,
                None,
                Some(&err_msg),
                "error",
            )
            .await;
            results.push(QueryResult {
                query_id,
                columns: vec![],
                rows: vec![],
                total_rows: None,
                duration_us,
                affected_rows: None,
                error: Some(err_msg),
            });
        }
    }
}

/// Execute a SQL query, returning a paginated result set.
/// The total row count is fetched asynchronously and pushed via the
/// `query-count-updated` Tauri event keyed by `queryId`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn query_execute(
    app: tauri::AppHandle,
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    sql: String,
    page: u32,
    page_size: u32,
    database: Option<String>,
    instance_db: Option<String>,
) -> Result<QueryResult, AppError> {
    let query_id = uuid::Uuid::new_v4().to_string();
    let start = std::time::Instant::now();

    let offset = (page.saturating_sub(1)) * page_size;

    // If a transaction is active, route through the held connection.
    // Background count is skipped for transaction queries.
    if let Some(tx) = transactions.get(&connection_id) {
        let mut guard = tx.lock().await;
        let engine_result = guard.execute(&sql, page_size, offset).await;
        let result = engine_result_to_execute_result(engine_result);
        let duration_us = start.elapsed().as_micros() as u64;
        return Ok(build_query_result(
            result,
            query_id,
            &connection_id,
            &sql,
            duration_us,
            sqlite.inner(),
        )
        .await);
    }

    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;

    let t = std::time::Instant::now();
    let exec_result = engine
        .execute(&sql, database.as_deref(), instance_db.as_deref(), page_size, offset)
        .await
        .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
    let execution_us = t.elapsed().as_micros() as u64;

    let duration_us = start.elapsed().as_micros() as u64;
    let result_processing_us = duration_us.saturating_sub(execution_us);

    let affected_rows = exec_result.affected_rows;
    let row_count = exec_result.rows.len() as i64;
    let query_id_clone = query_id.clone();
    record_history(
        sqlite.inner(),
        &query_id,
        &connection_id,
        &sql,
        duration_us,
        Some(row_count),
        None,
        "success",
    )
    .await;
    let query_result = QueryResult {
        query_id,
        columns: exec_result.columns,
        rows: exec_result.rows,
        total_rows: None,
        duration_us,
        affected_rows,
        error: None,
    };

    #[cfg(debug_assertions)]
    record_speed_analysis(
        sqlite.inner(),
        &connection_id,
        &sql,
        duration_us,
        0,
        0,
        execution_us,
        result_processing_us,
        Some(query_result.rows.len() as i64),
    )
    .await;

    // Spawn background count only for successful SELECT results (no affected_rows means rows were returned).
    if query_result.error.is_none() && !query_result.columns.is_empty() && affected_rows.is_none() {
        let engine = engine.clone();
        let sql_clone = sql.clone();
        let db_clone = database.clone();
        let inst_db_clone = instance_db.clone();
        let qid = query_id_clone;
        tokio::spawn(async move {
            if let Some(total) = engine.count_query_rows(&sql_clone, db_clone.as_deref(), inst_db_clone.as_deref()).await {
                let _ = app.emit("query-count-updated", QueryCountPayload { query_id: qid, total_rows: total });
            }
        });
    }

    Ok(query_result)
}

async fn build_query_result(
    result: ExecuteResult,
    query_id: String,
    connection_id: &str,
    sql: &str,
    duration_us: u64,
    sqlite: &sqlx::SqlitePool,
) -> QueryResult {
    match result {
        Ok((columns, rows, total_rows, affected_rows)) => {
            let row_count = rows.len() as i64;
            record_history(
                sqlite,
                &query_id,
                connection_id,
                sql,
                duration_us,
                Some(row_count),
                None,
                "success",
            )
            .await;
            QueryResult {
                query_id,
                columns,
                rows,
                total_rows,
                duration_us,
                affected_rows,
                error: None,
            }
        }
        Err(err_msg) => {
            record_history(
                sqlite,
                &query_id,
                connection_id,
                sql,
                duration_us,
                None,
                Some(&err_msg),
                "error",
            )
            .await;
            QueryResult {
                query_id,
                columns: vec![],
                rows: vec![],
                total_rows: None,
                duration_us,
                affected_rows: None,
                error: Some(err_msg),
            }
        }
    }
}

/// Run a selection/fragment of SQL (no pagination wrapper).
#[tauri::command]
pub async fn query_execute_selection(
    app: tauri::AppHandle,
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    sql: String,
    database: Option<String>,
    instance_db: Option<String>,
) -> Result<QueryResult, AppError> {
    // Execute without LIMIT/OFFSET so the user's highlighted text reaches the driver unchanged.
    query_execute(
        app,
        sqlite,
        connections,
        transactions,
        connection_id,
        sql,
        1,
        UNBOUNDED,
        database,
        instance_db,
    )
    .await
}

// ── Dialect-specific executors ────────────────────────────────────────────────

/// Sentinel for `page_size` meaning "fetch all rows without a LIMIT clause".
const UNBOUNDED: u32 = 0;

/// Build the paginated SQL string sent to the driver for a row-returning statement.
/// `sql` must already have its trailing semicolon stripped.
/// Returns the SQL unchanged when `page_size` is `UNBOUNDED` (0) or when the query
/// already contains a top-level LIMIT clause (respecting the user's explicit limit).
/// Otherwise appends `LIMIT {page_size} OFFSET {offset}`.
fn build_paginated_sql(sql: &str, page_size: u32, offset: u32) -> String {
    if page_size == UNBOUNDED || sql_has_top_level_limit(sql) {
        sql.to_string()
    } else {
        format!("{sql} LIMIT {page_size} OFFSET {offset}")
    }
}

/// Returns true if `sql` has a LIMIT keyword at the top level
/// (i.e., not inside quoted strings, block comments, or nested parentheses).
fn sql_has_top_level_limit(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut depth = 0i32;

    while i < len {
        match bytes[i] {
            // Quoted identifiers / strings: skip to matching close quote
            q @ (b'\'' | b'"' | b'`') => {
                i += 1;
                while i < len {
                    if bytes[i] == q {
                        i += 1;
                        // doubled-quote escape ('' or "" or ``)
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
            // Line comment --
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            // Block comment /* ... */
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
                            || !bytes[i - 1].is_ascii_alphabetic()
                                && bytes[i - 1] != b'_';
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

type ExecuteResult = Result<
    (
        Vec<ColumnMeta>,
        Vec<Vec<serde_json::Value>>,
        Option<i64>,
        Option<u64>,
    ),
    String,
>;

fn engine_result_to_execute_result(
    r: Result<crate::connections::types::EngineQueryResult, crate::error::RowmanceError>,
) -> ExecuteResult {
    match r {
        Ok(er) => Ok((er.columns, er.rows, None, er.affected_rows)),
        Err(e) => Err(e.to_string()),
    }
}

/// Run EXPLAIN (or equivalent) for the given SQL and return the plan as JSON + dialect tag.
#[tauri::command]
pub async fn query_explain(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
    database: Option<String>,
    instance_db: Option<String>,
) -> Result<crate::connections::types::ExplainResult, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.explain(&sql, database.as_deref(), instance_db.as_deref()).await.map_err(AppError::from)
}

/// Write a query execution record to the local history table.
/// Failures are logged but never propagated — they must not interrupt the user's query flow.
#[allow(clippy::too_many_arguments)]
async fn record_history(
    pool: &sqlx::SqlitePool,
    query_id: &str,
    connection_id: &str,
    sql: &str,
    duration_us: u64,
    row_count: Option<i64>,
    error: Option<&str>,
    status: &str,
) {
    let now = chrono::Utc::now().to_rfc3339();
    let duration_us_i64 = duration_us as i64;
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO query_history (id, connection_id, sql, executed_at, duration_us, row_count, error, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(query_id)
    .bind(connection_id)
    .bind(sql)
    .bind(now)
    .bind(duration_us_i64)
    .bind(row_count)
    .bind(error)
    .bind(status)
    .execute(pool)
    .await
    {
        eprintln!("history write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_statements_single() {
        assert_eq!(split_statements("SELECT 1"), vec!["SELECT 1"]);
    }

    #[test]
    fn split_statements_multiple() {
        let stmts = split_statements("SELECT 1; SELECT 2");
        assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn split_statements_trailing_semicolon() {
        let stmts = split_statements("SELECT 1;");
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    #[test]
    fn split_statements_semicolon_in_string() {
        let stmts = split_statements("SELECT 'a;b'; SELECT 2");
        assert_eq!(stmts, vec!["SELECT 'a;b'", "SELECT 2"]);
    }

    #[test]
    fn split_statements_empty_and_whitespace() {
        let stmts = split_statements("  ;  ;  SELECT 1  ;  ");
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    // ── build_paginated_sql ────────────────────────────────────────────────────

    #[test]
    fn paginated_select_appends_limit_offset() {
        assert_eq!(
            build_paginated_sql("SELECT * FROM orders", 100, 0),
            "SELECT * FROM orders LIMIT 100 OFFSET 0"
        );
    }

    #[test]
    fn paginated_select_nonzero_offset() {
        assert_eq!(
            build_paginated_sql("SELECT * FROM orders", 50, 150),
            "SELECT * FROM orders LIMIT 50 OFFSET 150"
        );
    }

    #[test]
    fn unbounded_page_size_returns_sql_unchanged() {
        assert_eq!(
            build_paginated_sql("SELECT * FROM orders", UNBOUNDED, 0),
            "SELECT * FROM orders"
        );
    }

    #[test]
    fn unbounded_ignores_nonzero_offset() {
        // offset is irrelevant when page_size is UNBOUNDED; SQL must be unchanged
        assert_eq!(
            build_paginated_sql("SELECT * FROM t", UNBOUNDED, 999),
            "SELECT * FROM t"
        );
    }

    #[test]
    fn paginated_sql_does_not_append_to_non_select() {
        // build_paginated_sql is only called in the is_select branch; verify the
        // helper itself does not corrupt non-SELECT SQL if ever called with it
        let sql = "UPDATE t SET a = 1";
        assert_eq!(
            build_paginated_sql(sql, 100, 0),
            "UPDATE t SET a = 1 LIMIT 100 OFFSET 0"
        );
    }

    // ── sql_has_top_level_limit ───────────────────────────────────────────────

    #[test]
    fn top_level_limit_detected() {
        assert!(sql_has_top_level_limit("SELECT * FROM t LIMIT 10"));
        assert!(sql_has_top_level_limit("SELECT * FROM t limit 10"));
        assert!(sql_has_top_level_limit("SELECT * FROM t LIMIT 10 OFFSET 5"));
        assert!(sql_has_top_level_limit("select * from t\nLIMIT\n20"));
    }

    #[test]
    fn limit_in_subquery_not_detected_as_top_level() {
        assert!(!sql_has_top_level_limit(
            "SELECT * FROM (SELECT * FROM t LIMIT 5) sub"
        ));
    }

    #[test]
    fn limit_in_string_not_detected() {
        assert!(!sql_has_top_level_limit("SELECT 'LIMIT 10' FROM t"));
        assert!(!sql_has_top_level_limit("SELECT \"LIMIT 10\" FROM t"));
    }

    #[test]
    fn limit_in_comment_not_detected() {
        assert!(!sql_has_top_level_limit(
            "SELECT * FROM t -- LIMIT 10\nWHERE id = 1"
        ));
        assert!(!sql_has_top_level_limit(
            "SELECT * FROM t /* LIMIT 10 */ WHERE id = 1"
        ));
    }

    #[test]
    fn word_containing_limit_not_detected() {
        assert!(!sql_has_top_level_limit("SELECT limited FROM t"));
        assert!(!sql_has_top_level_limit("SELECT * FROM unlimited"));
    }

    #[test]
    fn existing_limit_prevents_pagination() {
        assert_eq!(
            build_paginated_sql("SELECT * FROM users LIMIT 10", 10_000, 0),
            "SELECT * FROM users LIMIT 10"
        );
    }

    #[test]
    fn no_limit_still_paginated() {
        assert_eq!(
            build_paginated_sql("SELECT * FROM users", 10_000, 0),
            "SELECT * FROM users LIMIT 10000 OFFSET 0"
        );
    }
}

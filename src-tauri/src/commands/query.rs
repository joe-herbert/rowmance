/// Tauri commands for executing SQL queries against remote databases.
use serde::{Deserialize, Serialize};
use sqlx::{Column, ConnectOptions, Executor, Row, Statement, TypeInfo};
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;

/// Column metadata included with every result set.
#[derive(Debug, Serialize, Clone)]
pub struct ColumnMeta {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
    #[serde(rename = "isForeignKey")]
    pub is_foreign_key: bool,
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
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    #[serde(rename = "affectedRows")]
    pub affected_rows: Option<u64>,
    pub error: Option<String>,
}

/// A single row's worth of changes sent from the frontend for an inline edit.
#[derive(Deserialize, Debug)]
pub struct RowChange {
    /// The primary key value(s) that identify the row. Key = column name, Value = JSON value.
    #[serde(rename = "primaryKeys")]
    pub primary_keys: std::collections::HashMap<String, serde_json::Value>,
    /// Column name → new value for each changed cell.
    pub changes: std::collections::HashMap<String, serde_json::Value>,
}

/// Result returned after executing a batch of row updates.
#[derive(Serialize, Debug)]
pub struct UpdateResult {
    #[serde(rename = "updatedCount")]
    pub updated_count: u64,
}

/// Quote an identifier for MySQL/MariaDB (backticks).
fn quote_mysql(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}

/// Quote an identifier for PostgreSQL (double-quotes).
fn quote_postgres(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Quote an identifier for SQLite (double-quotes, ANSI SQL).
fn quote_sqlite(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Bind a JSON value to a SQLx MySQL query.
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

/// Bind a JSON value to a SQLx Postgres query.
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

/// Bind a JSON value to a SQLx SQLite query.
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

/// A row to delete, identified by its primary key column values.
#[derive(Deserialize, Debug)]
pub struct RowDelete {
    #[serde(rename = "primaryKeys")]
    pub primary_keys: std::collections::HashMap<String, serde_json::Value>,
}

/// Execute a batch of row-level DELETE statements.
#[tauri::command]
pub async fn query_delete_rows(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    table: String,
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

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    let mut total_deleted: u64 = 0;

    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            for row_del in &rows {
                if row_del.primary_keys.is_empty() {
                    continue;
                }
                let where_pairs: Vec<(&String, &serde_json::Value)> =
                    row_del.primary_keys.iter().collect();
                let mut where_parts: Vec<String> = Vec::new();
                let mut where_bind: Vec<&serde_json::Value> = Vec::new();
                for (col, val) in &where_pairs {
                    if val.is_null() {
                        where_parts.push(format!("{} IS NULL", quote_mysql(col)));
                    } else {
                        where_parts.push(format!("{} = ?", quote_mysql(col)));
                        where_bind.push(val);
                    }
                }
                let sql = format!(
                    "DELETE FROM {}.{} WHERE {} LIMIT 1",
                    quote_mysql(&database),
                    quote_mysql(&table),
                    where_parts.join(" AND ")
                );
                let mut q = sqlx::query(&sql);
                for val in &where_bind {
                    q = bind_mysql_value(q, val);
                }
                let result = q
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                total_deleted += result.rows_affected();
            }
        }
        RemotePool::Postgres(pool) => {
            for row_del in &rows {
                if row_del.primary_keys.is_empty() {
                    continue;
                }
                let where_pairs: Vec<(&String, &serde_json::Value)> =
                    row_del.primary_keys.iter().collect();
                let mut param_idx: usize = 1;
                let mut where_parts: Vec<String> = Vec::new();
                let mut where_bind: Vec<&serde_json::Value> = Vec::new();
                for (col, val) in &where_pairs {
                    if val.is_null() {
                        where_parts.push(format!("{} IS NULL", quote_postgres(col)));
                    } else {
                        where_parts.push(format!("{} = ${}", quote_postgres(col), param_idx));
                        param_idx += 1;
                        where_bind.push(val);
                    }
                }
                let sql = format!(
                    "DELETE FROM {}.{} WHERE {}",
                    quote_postgres(&database),
                    quote_postgres(&table),
                    where_parts.join(" AND ")
                );
                let mut q = sqlx::query(&sql);
                for val in &where_bind {
                    q = bind_pg_value(q, val);
                }
                let result = q
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                total_deleted += result.rows_affected();
            }
        }
        RemotePool::Sqlite(pool) => {
            for row_del in &rows {
                if row_del.primary_keys.is_empty() {
                    continue;
                }
                let where_pairs: Vec<(&String, &serde_json::Value)> =
                    row_del.primary_keys.iter().collect();
                let mut where_parts: Vec<String> = Vec::new();
                let mut where_bind: Vec<&serde_json::Value> = Vec::new();
                for (col, val) in &where_pairs {
                    if val.is_null() {
                        where_parts.push(format!("{} IS NULL", quote_sqlite(col)));
                    } else {
                        where_parts.push(format!("{} = ?", quote_sqlite(col)));
                        where_bind.push(val);
                    }
                }
                let sql = format!(
                    "DELETE FROM {} WHERE {}",
                    quote_sqlite(&table),
                    where_parts.join(" AND ")
                );
                let mut q = sqlx::query(&sql);
                for val in &where_bind {
                    q = bind_sqlite_value(q, val);
                }
                let result = q
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                total_deleted += result.rows_affected();
            }
        }
    }

    Ok(UpdateResult { updated_count: total_deleted })
}

/// Execute a batch of row-level UPDATE statements for inline cell editing.
#[tauri::command]
pub async fn query_update_rows(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    table: String,
    database: String,
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

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    let mut total_updated: u64 = 0;

    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            for change in &changes {
                if change.changes.is_empty() || change.primary_keys.is_empty() {
                    continue;
                }

                let set_clause: Vec<String> = change
                    .changes
                    .keys()
                    .map(|col| format!("{} = ?", quote_mysql(col)))
                    .collect();

                // Collect WHERE entries once so iteration order is consistent between
                // clause generation and binding.
                let where_pairs: Vec<(&String, &serde_json::Value)> =
                    change.primary_keys.iter().collect();
                let mut where_parts: Vec<String> = Vec::new();
                let mut where_bind: Vec<&serde_json::Value> = Vec::new();
                for (col, val) in &where_pairs {
                    if val.is_null() {
                        where_parts.push(format!("{} IS NULL", quote_mysql(col)));
                    } else {
                        where_parts.push(format!("{} = ?", quote_mysql(col)));
                        where_bind.push(val);
                    }
                }

                // LIMIT 1 guards against accidentally updating multiple rows on
                // tables without a primary key (all-column WHERE match).
                let sql = format!(
                    "UPDATE {}.{} SET {} WHERE {} LIMIT 1",
                    quote_mysql(&database),
                    quote_mysql(&table),
                    set_clause.join(", "),
                    where_parts.join(" AND ")
                );

                let mut q = sqlx::query(&sql);
                for val in change.changes.values() {
                    q = bind_mysql_value(q, val);
                }
                for val in where_bind {
                    q = bind_mysql_value(q, val);
                }

                let result = q
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                total_updated += result.rows_affected();
            }
        }
        RemotePool::Postgres(pool) => {
            for change in &changes {
                if change.changes.is_empty() || change.primary_keys.is_empty() {
                    continue;
                }

                let mut param_idx: usize = 1;

                let set_clause: Vec<String> = change
                    .changes
                    .keys()
                    .map(|col| {
                        let s = format!("{} = ${}", quote_postgres(col), param_idx);
                        param_idx += 1;
                        s
                    })
                    .collect();

                let where_pairs: Vec<(&String, &serde_json::Value)> =
                    change.primary_keys.iter().collect();
                let mut where_parts: Vec<String> = Vec::new();
                let mut where_bind: Vec<&serde_json::Value> = Vec::new();
                for (col, val) in &where_pairs {
                    if val.is_null() {
                        where_parts.push(format!("{} IS NULL", quote_postgres(col)));
                    } else {
                        where_parts.push(format!("{} = ${}", quote_postgres(col), param_idx));
                        param_idx += 1;
                        where_bind.push(val);
                    }
                }

                let sql = format!(
                    "UPDATE {}.{} SET {} WHERE {}",
                    quote_postgres(&database),
                    quote_postgres(&table),
                    set_clause.join(", "),
                    where_parts.join(" AND ")
                );

                let mut q = sqlx::query(&sql);
                for val in change.changes.values() {
                    q = bind_pg_value(q, val);
                }
                for val in where_bind {
                    q = bind_pg_value(q, val);
                }

                let result = q
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                total_updated += result.rows_affected();
            }
        }
        RemotePool::Sqlite(pool) => {
            for change in &changes {
                if change.changes.is_empty() || change.primary_keys.is_empty() {
                    continue;
                }

                let set_clause: Vec<String> = change
                    .changes
                    .keys()
                    .map(|col| format!("{} = ?", quote_sqlite(col)))
                    .collect();

                let where_pairs: Vec<(&String, &serde_json::Value)> =
                    change.primary_keys.iter().collect();
                let mut where_parts: Vec<String> = Vec::new();
                let mut where_bind: Vec<&serde_json::Value> = Vec::new();
                for (col, val) in &where_pairs {
                    if val.is_null() {
                        where_parts.push(format!("{} IS NULL", quote_sqlite(col)));
                    } else {
                        where_parts.push(format!("{} = ?", quote_sqlite(col)));
                        where_bind.push(val);
                    }
                }

                let sql = format!(
                    "UPDATE {} SET {} WHERE {}",
                    quote_sqlite(&table),
                    set_clause.join(", "),
                    where_parts.join(" AND ")
                );

                let mut q = sqlx::query(&sql);
                for val in change.changes.values() {
                    q = bind_sqlite_value(q, val);
                }
                for val in where_bind {
                    q = bind_sqlite_value(q, val);
                }

                let result = q
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                total_updated += result.rows_affected();
            }
        }
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
    connection_id: String,
    database: String,
    table: String,
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
        return Err(AppError::new("INVALID_INPUT", "No values provided for insert"));
    }

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    // Collect columns and values in a stable order.
    let cols: Vec<(&String, &serde_json::Value)> = values.iter().collect();

    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let col_list: Vec<String> = cols.iter().map(|(c, _)| quote_mysql(c)).collect();
            let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
            let sql = format!(
                "INSERT INTO {}.{} ({}) VALUES ({})",
                quote_mysql(&database),
                quote_mysql(&table),
                col_list.join(", "),
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&sql);
            for (_, val) in &cols {
                q = bind_mysql_value(q, val);
            }
            q.execute(pool)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        RemotePool::Postgres(pool) => {
            let col_list: Vec<String> = cols.iter().map(|(c, _)| quote_postgres(c)).collect();
            let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
            let sql = format!(
                "INSERT INTO {}.{} ({}) VALUES ({})",
                quote_postgres(&database),
                quote_postgres(&table),
                col_list.join(", "),
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&sql);
            for (_, val) in &cols {
                q = bind_pg_value(q, val);
            }
            q.execute(pool)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        RemotePool::Sqlite(pool) => {
            let col_list: Vec<String> = cols.iter().map(|(c, _)| quote_sqlite(c)).collect();
            let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                quote_sqlite(&table),
                col_list.join(", "),
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&sql);
            for (_, val) in &cols {
                q = bind_sqlite_value(q, val);
            }
            q.execute(pool)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
    }

    Ok(())
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
pub async fn query_execute_multi(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
    database: Option<String>,
) -> Result<Vec<QueryResult>, AppError> {
    let statements = split_statements(&sql);
    if statements.is_empty() {
        return Ok(vec![]);
    }

    let profile_row = sqlx::query!(
        "SELECT read_only FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let is_read_only = profile_row.as_ref().map(|r| r.read_only != 0).unwrap_or(false);

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    let mut results: Vec<QueryResult> = Vec::new();

    for stmt in &statements {
        if is_read_only && is_mutating_statement(stmt) {
            results.push(QueryResult {
                query_id: uuid::Uuid::new_v4().to_string(),
                columns: vec![],
                rows: vec![],
                total_rows: None,
                duration_ms: 0,
                affected_rows: None,
                error: Some(
                    "This connection is in read-only mode — mutating statements are not allowed"
                        .to_string(),
                ),
            });
            continue;
        }

        let query_id = uuid::Uuid::new_v4().to_string();
        let start = std::time::Instant::now();

        let exec_result = match pool_ref.value() {
            RemotePool::MySql(pool) => {
                if let Some(db) = &database {
                    let mut conn = (*pool.connect_options()).clone().database(db)
                        .connect().await
                        .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
                    execute_mysql(&mut conn, stmt, 10_000, 0).await
                } else {
                    let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
                    execute_mysql(&mut *conn, stmt, 10_000, 0).await
                }
            }
            RemotePool::Postgres(pool) => {
                let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
                if let Some(schema) = &database {
                    sqlx::query(&format!("SET search_path = {}", quote_postgres(schema)))
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                }
                execute_postgres(&mut *conn, stmt, 10_000, 0).await
            }
            RemotePool::Sqlite(pool) => {
                let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
                execute_sqlite(&mut *conn, stmt, 10_000, 0).await
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match exec_result {
            Ok((columns, rows, total_rows, affected_rows)) => {
                let row_count = rows.len() as i64;
                record_history(
                    sqlite.inner(),
                    &query_id,
                    &connection_id,
                    stmt,
                    duration_ms,
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
                    duration_ms,
                    affected_rows,
                    error: None,
                });
            }
            Err(err_msg) => {
                record_history(
                    sqlite.inner(),
                    &query_id,
                    &connection_id,
                    stmt,
                    duration_ms,
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
                    duration_ms,
                    affected_rows: None,
                    error: Some(err_msg),
                });
            }
        }
    }

    Ok(results)
}

/// Enforce read-only mode by checking the leading SQL keyword.
fn is_mutating_statement(sql: &str) -> bool {
    let keyword = sql.split_whitespace().next().unwrap_or("").to_uppercase();
    matches!(
        keyword.as_str(),
        "INSERT"
            | "UPDATE"
            | "DELETE"
            | "DROP"
            | "CREATE"
            | "ALTER"
            | "TRUNCATE"
            | "REPLACE"
            | "MERGE"
            | "RENAME"
    )
}

/// Execute a SQL query, returning a paginated result set.
#[tauri::command]
pub async fn query_execute(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    #[allow(unused_variables)] connection_id: String,
    sql: String,
    page: u32,
    page_size: u32,
    database: Option<String>,
) -> Result<QueryResult, AppError> {
    let query_id = uuid::Uuid::new_v4().to_string();
    let start = std::time::Instant::now();

    // Check read-only mode before touching the remote pool.
    let profile_row = sqlx::query!(
        "SELECT read_only FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    if let Some(row) = &profile_row {
        if row.read_only != 0 && is_mutating_statement(&sql) {
            return Err(AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode — mutating statements are not allowed",
            ));
        }
    }

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    let offset = (page.saturating_sub(1)) * page_size;

    let result = match pool_ref.value() {
        RemotePool::MySql(pool) => {
            if let Some(db) = &database {
                // `USE db` cannot be sent as a prepared statement (MySQL error 1295).
                // Open a short-lived direct connection with the target database set in
                // the options so we never need to issue USE at all.
                let mut conn = (*pool.connect_options()).clone().database(db)
                    .connect().await
                    .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
                execute_mysql(&mut conn, &sql, page_size, offset).await
            } else {
                let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
                execute_mysql(&mut *conn, &sql, page_size, offset).await
            }
        }
        RemotePool::Postgres(pool) => {
            let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            if let Some(schema) = &database {
                sqlx::query(&format!("SET search_path = {}", quote_postgres(schema)))
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            }
            execute_postgres(&mut *conn, &sql, page_size, offset).await
        }
        RemotePool::Sqlite(pool) => {
            let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            execute_sqlite(&mut *conn, &sql, page_size, offset).await
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok((columns, rows, total_rows, affected_rows)) => {
            let row_count = rows.len() as i64;
            record_history(
                sqlite.inner(),
                &query_id,
                &connection_id,
                &sql,
                duration_ms,
                Some(row_count),
                None,
                "success",
            )
            .await;

            Ok(QueryResult {
                query_id,
                columns,
                rows,
                total_rows,
                duration_ms,
                affected_rows,
                error: None,
            })
        }
        Err(err_msg) => {
            record_history(
                sqlite.inner(),
                &query_id,
                &connection_id,
                &sql,
                duration_ms,
                None,
                Some(&err_msg),
                "error",
            )
            .await;

            Ok(QueryResult {
                query_id,
                columns: vec![],
                rows: vec![],
                total_rows: None,
                duration_ms,
                affected_rows: None,
                error: Some(err_msg),
            })
        }
    }
}

/// Run a selection/fragment of SQL (no pagination wrapper).
#[tauri::command]
pub async fn query_execute_selection(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
    database: Option<String>,
) -> Result<QueryResult, AppError> {
    // Delegate to the main executor with page=1 and a large page size.
    query_execute(sqlite, connections, connection_id, sql, 1, 10_000, database).await
}

// ── Type formatting helpers ───────────────────────────────────────────────────

fn format_mysql_type(type_info: &sqlx::mysql::MySqlTypeInfo) -> String {
    let base = type_info.name().to_lowercase();
    // max_size is pub(crate) in sqlx, so extract it from the Debug representation.
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

// ── Dialect-specific executors ────────────────────────────────────────────────

type ExecuteResult = Result<
    (
        Vec<ColumnMeta>,
        Vec<Vec<serde_json::Value>>,
        Option<i64>,
        Option<u64>,
    ),
    String,
>;

async fn execute_mysql(
    conn: &mut sqlx::mysql::MySqlConnection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> ExecuteResult {
    // Wrap SELECT statements with LIMIT/OFFSET; pass DDL/DML through unchanged.
    let sql = sql.trim_end_matches(';');
    let is_select = sql.trim().to_uppercase().starts_with("SELECT");

    if is_select {
        let count_sql = format!("SELECT COUNT(*) FROM ({sql}) AS _count_query");
        let total_rows: Option<i64> = sqlx::query(&count_sql)
            .fetch_one(&mut *conn)
            .await
            .ok()
            .and_then(|row| row.try_get::<i64, _>(0).ok());

        let paginated = format!("{sql} LIMIT {page_size} OFFSET {offset}");
        let rows = sqlx::query(&paginated)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;

        let columns = if let Some(first) = rows.first() {
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

        Ok((columns, data, total_rows, None))
    } else {
        let result = sqlx::query(sql)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok((vec![], vec![], None, Some(result.rows_affected())))
    }
}

async fn execute_postgres(
    conn: &mut sqlx::postgres::PgConnection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> ExecuteResult {
    let sql = sql.trim_end_matches(';');
    let is_select = sql.trim().to_uppercase().starts_with("SELECT");

    if is_select {
        let count_sql = format!("SELECT COUNT(*) FROM ({sql}) AS _count_query");
        let total_rows: Option<i64> = sqlx::query(&count_sql)
            .fetch_one(&mut *conn)
            .await
            .ok()
            .and_then(|row| row.try_get::<i64, _>(0).ok());

        let paginated = format!("{sql} LIMIT {page_size} OFFSET {offset}");
        let rows = sqlx::query(&paginated)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;

        let columns = if let Some(first) = rows.first() {
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

        Ok((columns, data, total_rows, None))
    } else {
        let result = sqlx::query(sql)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok((vec![], vec![], None, Some(result.rows_affected())))
    }
}

async fn execute_sqlite(
    conn: &mut sqlx::sqlite::SqliteConnection,
    sql: &str,
    page_size: u32,
    offset: u32,
) -> ExecuteResult {
    let sql = sql.trim_end_matches(';');
    let is_select = sql.trim().to_uppercase().starts_with("SELECT");

    if is_select {
        let count_sql = format!("SELECT COUNT(*) FROM ({sql}) AS _count_query");
        let total_rows: Option<i64> = sqlx::query(&count_sql)
            .fetch_one(&mut *conn)
            .await
            .ok()
            .and_then(|row| row.try_get::<i64, _>(0).ok());

        let paginated = format!("{sql} LIMIT {page_size} OFFSET {offset}");
        let rows = sqlx::query(&paginated)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;

        let columns = if let Some(first) = rows.first() {
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
            .map(|row| (0..row.len()).map(|i| sqlite_value_to_json(row, i)).collect())
            .collect();

        Ok((columns, data, total_rows, None))
    } else {
        let result = sqlx::query(sql)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok((vec![], vec![], None, Some(result.rows_affected())))
    }
}

/// Convert a SQLite column value to a JSON-serialisable form.
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

/// Convert a MySQL column value to a JSON-serialisable form.
/// Falls back to a string representation for types that don't map cleanly.
fn mysql_value_to_json(row: &sqlx::mysql::MySqlRow, idx: usize) -> serde_json::Value {
    use sqlx::Column;
    // For bool / tinyint(1) columns, decode as bool before trying numeric types.
    // Without this, try_get::<i64> succeeds for tinyint(1) and returns 0/1 instead of false/true.
    let col = &row.columns()[idx];
    let type_name = col.type_info().name().to_lowercase();
    let debug = format!("{:?}", col.type_info());
    let is_bool_col = type_name == "boolean"
        || (type_name == "tinyint" && parse_max_size(&debug) == Some(1));
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
    // MySQL TIME can be negative or > 23:59:59; NaiveTime rejects those, fall back to MySqlTime
    if let Ok(v) = row.try_get::<Option<sqlx::mysql::types::MySqlTime>, _>(idx) {
        return v
            .map(|t| serde_json::Value::String(t.to_string()))
            .unwrap_or(serde_json::Value::Null);
    }
    // MySQL YEAR type decodes as u16
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

/// Convert a PostgreSQL column value to a JSON-serialisable form.
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
    if let Ok(v) = row.try_get::<Option<sqlx::postgres::types::PgTimeTz<chrono::NaiveTime, chrono::FixedOffset>>, _>(idx) {
        return v
            .map(|tz| {
                let offset = tz.offset.local_minus_utc();
                let (sign, abs_secs) = if offset >= 0 { ('+', offset as u32) } else { ('-', (-offset) as u32) };
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
                if interval.months != 0 { parts.push(format!("{} months", interval.months)); }
                if interval.days != 0 { parts.push(format!("{} days", interval.days)); }
                if interval.microseconds != 0 { parts.push(format!("{} μs", interval.microseconds)); }
                serde_json::Value::String(if parts.is_empty() { "0".to_string() } else { parts.join(" ") })
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

/// Result from running EXPLAIN on a query.
#[derive(Debug, Serialize)]
pub struct ExplainResult {
    #[serde(rename = "rawJson")]
    pub raw_json: String,
    pub dialect: String,
}

/// Run EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) for Postgres or EXPLAIN FORMAT=JSON for MySQL.
#[tauri::command]
pub async fn query_explain(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
    database: Option<String>,
) -> Result<ExplainResult, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let base_opts = (*pool.connect_options()).clone();
            let opts = if let Some(db) = &database { base_opts.database(db) } else { base_opts };
            let mut conn = opts.connect().await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            let explain_sql = format!("EXPLAIN FORMAT=JSON {sql}");
            let rows = sqlx::query(&explain_sql)
                .fetch_all(&mut conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            let raw = if let Some(row) = rows.first() {
                use sqlx::Row;
                row.try_get::<String, _>(0).unwrap_or_default()
            } else {
                String::new()
            };
            Ok(ExplainResult {
                raw_json: raw,
                dialect: "mysql".to_string(),
            })
        }
        RemotePool::Postgres(pool) => {
            let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            if let Some(schema) = &database {
                sqlx::query(&format!("SET search_path = {}", quote_postgres(schema)))
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            }
            let explain_sql = format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {sql}");
            let rows = sqlx::query(&explain_sql)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
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
                    .map_err(|e| AppError::new("SERIALISATION_ERROR", e.to_string()))?,
                dialect: "postgres".to_string(),
            })
        }
        RemotePool::Sqlite(pool) => {
            let mut conn = pool.acquire().await.map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
            let rows = sqlx::query(&explain_sql)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
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
                    .map_err(|e| AppError::new("SERIALISATION_ERROR", e.to_string()))?,
                dialect: "sqlite".to_string(),
            })
        }
    }
}

/// Format a SQL string. Actual formatting is done by the frontend via sql-formatter;
/// this command exists for API completeness and returns the SQL unchanged.
#[tauri::command]
pub async fn query_format(sql: String, _dialect: String) -> Result<String, AppError> {
    Ok(sql)
}

/// Write a query execution record to the local history table.
/// This is fire-and-forget — failures are swallowed so they never
/// interrupt the user's query flow.
#[allow(clippy::too_many_arguments)]
async fn record_history(
    pool: &sqlx::SqlitePool,
    query_id: &str,
    connection_id: &str,
    sql: &str,
    duration_ms: u64,
    row_count: Option<i64>,
    error: Option<&str>,
    status: &str,
) {
    let now = chrono::Utc::now().to_rfc3339();
    let duration_ms_i64 = duration_ms as i64;
    let _ = sqlx::query!(
        r#"
        INSERT INTO query_history (id, connection_id, sql, executed_at, duration_ms, row_count, error, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        query_id,
        connection_id,
        sql,
        now,
        duration_ms_i64,
        row_count,
        error,
        status
    )
    .execute(pool)
    .await;
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

    #[test]
    fn mutating_keywords_are_detected() {
        assert!(is_mutating_statement("INSERT INTO t VALUES (1)"));
        assert!(is_mutating_statement("UPDATE t SET a = 1"));
        assert!(is_mutating_statement("DELETE FROM t"));
        assert!(is_mutating_statement("DROP TABLE t"));
        assert!(is_mutating_statement("  truncate table t"));
        assert!(!is_mutating_statement("SELECT * FROM t"));
        assert!(!is_mutating_statement("EXPLAIN SELECT 1"));
    }

    #[test]
    fn all_mutating_keywords_covered() {
        for keyword in &["INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "TRUNCATE", "REPLACE", "MERGE", "RENAME"] {
            let sql = format!("{keyword} something");
            assert!(is_mutating_statement(&sql), "{keyword} should be mutating");
        }
    }

    #[test]
    fn case_insensitive_detection() {
        assert!(is_mutating_statement("insert into t values (1)"));
        assert!(is_mutating_statement("Insert Into t Values (1)"));
        assert!(!is_mutating_statement("select * from t"));
    }

    #[test]
    fn leading_whitespace_is_handled() {
        assert!(is_mutating_statement("   DELETE FROM t"));
        assert!(is_mutating_statement("\tUPDATE t SET a = 1"));
        assert!(!is_mutating_statement("   SELECT 1"));
    }

    #[test]
    fn empty_sql_is_not_mutating() {
        assert!(!is_mutating_statement(""));
        assert!(!is_mutating_statement("   "));
    }

    #[test]
    fn select_with_mutating_word_in_subquery_is_not_mutating() {
        // Only the leading keyword matters, not words in the middle.
        assert!(!is_mutating_statement("SELECT * FROM t WHERE id IN (SELECT id FROM deleted_items)"));
    }

    // ── Identifier quoting ────────────────────────────────────────────────────

    #[test]
    fn quote_mysql_wraps_in_backticks() {
        assert_eq!(quote_mysql("my_table"), "`my_table`");
    }

    #[test]
    fn quote_mysql_escapes_embedded_backticks() {
        assert_eq!(quote_mysql("ta`ble"), "`ta``ble`");
    }

    #[test]
    fn quote_postgres_wraps_in_double_quotes() {
        assert_eq!(quote_postgres("my_table"), "\"my_table\"");
    }

    #[test]
    fn quote_postgres_escapes_embedded_double_quotes() {
        assert_eq!(quote_postgres("ta\"ble"), "\"ta\"\"ble\"");
    }

    #[test]
    fn quote_mysql_handles_simple_identifiers() {
        assert_eq!(quote_mysql("users"), "`users`");
        assert_eq!(quote_mysql("created_at"), "`created_at`");
    }

    #[test]
    fn quote_postgres_handles_simple_identifiers() {
        assert_eq!(quote_postgres("users"), "\"users\"");
        assert_eq!(quote_postgres("created_at"), "\"created_at\"");
    }
}

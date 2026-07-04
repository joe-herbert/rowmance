/// Tauri commands for importing data from CSV or SQL files.
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct InferredColumn {
    pub name: String,
    #[serde(rename = "inferredType")]
    pub inferred_type: String,
}

#[derive(Debug, Serialize)]
pub struct CsvPreview {
    pub columns: Vec<InferredColumn>,
    #[serde(rename = "previewRows")]
    pub preview_rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ColumnOverride {
    pub name: String,
    #[serde(rename = "dbType")]
    pub db_type: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImportProgress {
    pub current: u32,
    pub total: Option<u32>,
    pub statement: String,
    pub error: Option<String>,
}

// ── Type inference ────────────────────────────────────────────────────────────

fn infer_type(values: &[&str]) -> String {
    let non_empty: Vec<&&str> = values.iter().filter(|v| !v.is_empty()).collect();
    if non_empty.is_empty() {
        return "text".to_owned();
    }

    let all_integer = non_empty.iter().all(|v| v.parse::<i64>().is_ok());
    if all_integer {
        return "integer".to_owned();
    }

    let all_float = non_empty.iter().all(|v| v.parse::<f64>().is_ok());
    if all_float {
        return "float".to_owned();
    }

    // Simple date check: YYYY-MM-DD
    let all_date = non_empty.iter().all(|v| {
        v.len() == 10
            && v.chars().nth(4) == Some('-')
            && v.chars().nth(7) == Some('-')
            && v[..4].parse::<u32>().is_ok()
            && v[5..7].parse::<u32>().is_ok()
            && v[8..10].parse::<u32>().is_ok()
    });
    if all_date {
        return "date".to_owned();
    }

    "text".to_owned()
}

// ── Commands ──────────────────────────────────────────────────────────────────

fn csv_preview_from_text(content: &str) -> Result<CsvPreview, AppError> {
    let mut reader = csv::Reader::from_reader(content.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| AppError::new("CSV_ERROR", e.to_string()))?
        .iter()
        .map(|h| h.to_owned())
        .collect();

    let mut preview_rows: Vec<Vec<String>> = Vec::new();
    let mut column_samples: Vec<Vec<String>> = vec![Vec::new(); headers.len()];

    for result in reader.records().take(20) {
        let record = result.map_err(|e| AppError::new("CSV_ERROR", e.to_string()))?;
        let row: Vec<String> = record.iter().map(|f| f.to_owned()).collect();
        for (i, val) in row.iter().enumerate() {
            if i < column_samples.len() {
                column_samples[i].push(val.clone());
            }
        }
        preview_rows.push(row);
    }

    let columns: Vec<InferredColumn> = headers
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let samples: Vec<&str> = column_samples[i].iter().map(|s| s.as_str()).collect();
            InferredColumn {
                name: name.clone(),
                inferred_type: infer_type(&samples),
            }
        })
        .collect();

    Ok(CsvPreview {
        columns,
        preview_rows,
    })
}

/// Read the first 20 rows of a CSV file and return column names with inferred types.
#[tauri::command]
pub async fn import_csv_preview(file_path: String) -> Result<CsvPreview, AppError> {
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::new("IO_ERROR", format!("Cannot read {file_path}: {e}")))?;
    csv_preview_from_text(&content)
}

/// Preview CSV data from a raw text string (e.g. clipboard content).
#[tauri::command]
pub async fn import_csv_preview_text(csv_text: String) -> Result<CsvPreview, AppError> {
    csv_preview_from_text(&csv_text)
}

async fn csv_execute_from_text(
    sqlite: &sqlx::SqlitePool,
    connections: &Arc<ConnectionManager>,
    connection_id: String,
    csv_text: String,
    table_name: String,
    create_table: bool,
    column_overrides: Vec<ColumnOverride>,
) -> Result<u32, AppError> {
    let profile_row = sqlx::query!(
        "SELECT read_only FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    match profile_row {
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ))
        }
        Some(row) if row.read_only != 0 => {
            return Err(AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode — mutating statements are not allowed",
            ))
        }
        _ => {}
    }

    let content = csv_text;
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| AppError::new("CSV_ERROR", e.to_string()))?
        .iter()
        .map(|h| h.to_owned())
        .collect();

    // Build effective column list (use overrides where supplied).
    let effective_columns: Vec<(String, String)> = headers
        .iter()
        .map(|h| {
            let db_type = column_overrides
                .iter()
                .find(|o| o.name == *h)
                .map(|o| o.db_type.clone())
                .unwrap_or_else(|| "TEXT".to_owned());
            (h.clone(), db_type)
        })
        .collect();

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    // Collect all records first to avoid holding the pool_ref across await points
    // with conflicting lifetime rules.
    let all_records: Vec<Vec<String>> = reader
        .records()
        .map(|r| r.map(|rec| rec.iter().map(|f| f.to_owned()).collect()))
        .collect::<Result<_, _>>()
        .map_err(|e| AppError::new("CSV_ERROR", e.to_string()))?;

    let mut inserted = 0u32;

    match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            if create_table {
                let col_defs: Vec<String> = effective_columns
                    .iter()
                    .map(|(name, db_type)| format!("`{}` {}", name.replace('`', "``"), db_type))
                    .collect();
                let ddl = format!(
                    "CREATE TABLE IF NOT EXISTS `{}` ({})",
                    table_name.replace('`', "``"),
                    col_defs.join(", ")
                );
                sqlx::query(&ddl)
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            }

            let col_names: Vec<String> = effective_columns
                .iter()
                .map(|(n, _)| format!("`{}`", n.replace('`', "``")))
                .collect();
            let placeholders: Vec<&str> = vec!["?"; col_names.len()];

            let sql = format!(
                "INSERT INTO `{}` ({}) VALUES ({})",
                table_name.replace('`', "``"),
                col_names.join(", "),
                placeholders.join(", ")
            );

            for record in &all_records {
                let mut q = sqlx::query(&sql);
                for field in record {
                    q = q.bind(field);
                }
                q.execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                inserted += 1;
            }
        }
        RemotePool::Postgres(pool) => {
            if create_table {
                let col_defs: Vec<String> = effective_columns
                    .iter()
                    .map(|(name, db_type)| format!("\"{}\" {}", name.replace('"', "\"\""), db_type))
                    .collect();
                let ddl = format!(
                    "CREATE TABLE IF NOT EXISTS \"{}\" ({})",
                    table_name.replace('"', "\"\""),
                    col_defs.join(", ")
                );
                sqlx::query(&ddl)
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            }

            let col_names: Vec<String> = effective_columns
                .iter()
                .map(|(n, _)| format!("\"{}\"", n.replace('"', "\"\"")))
                .collect();

            let mut param_idx = 1;
            let placeholders: Vec<String> = col_names
                .iter()
                .map(|_| {
                    let p = format!("${param_idx}");
                    param_idx += 1;
                    p
                })
                .collect();

            let sql = format!(
                "INSERT INTO \"{}\" ({}) VALUES ({})",
                table_name.replace('"', "\"\""),
                col_names.join(", "),
                placeholders.join(", ")
            );

            for record in &all_records {
                let mut q = sqlx::query(&sql);
                for field in record {
                    q = q.bind(field);
                }
                q.execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                inserted += 1;
            }
        }
        RemotePool::Sqlite(pool) => {
            if create_table {
                let col_defs: Vec<String> = effective_columns
                    .iter()
                    .map(|(name, db_type)| format!("\"{}\" {}", name.replace('"', "\"\""), db_type))
                    .collect();
                let ddl = format!(
                    "CREATE TABLE IF NOT EXISTS \"{}\" ({})",
                    table_name.replace('"', "\"\""),
                    col_defs.join(", ")
                );
                sqlx::query(&ddl)
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            }

            let col_names: Vec<String> = effective_columns
                .iter()
                .map(|(n, _)| format!("\"{}\"", n.replace('"', "\"\"")))
                .collect();
            let placeholders: Vec<&str> = vec!["?"; col_names.len()];

            let sql = format!(
                "INSERT INTO \"{}\" ({}) VALUES ({})",
                table_name.replace('"', "\"\""),
                col_names.join(", "),
                placeholders.join(", ")
            );

            for record in &all_records {
                let mut q = sqlx::query(&sql);
                for field in record {
                    q = q.bind(field.as_str());
                }
                q.execute(pool)
                    .await
                    .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
                inserted += 1;
            }
        }
    }

    Ok(inserted)
}

/// Import a CSV file into a database table.
#[tauri::command]
pub async fn import_csv_execute(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    file_path: String,
    table_name: String,
    create_table: bool,
    column_overrides: Vec<ColumnOverride>,
) -> Result<u32, AppError> {
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::new("IO_ERROR", format!("Cannot read {file_path}: {e}")))?;
    csv_execute_from_text(
        sqlite.inner(),
        connections.inner(),
        connection_id,
        content,
        table_name,
        create_table,
        column_overrides,
    )
    .await
}

/// Import CSV data from a raw text string (e.g. clipboard content).
#[tauri::command]
pub async fn import_csv_execute_text(
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    csv_text: String,
    table_name: String,
    create_table: bool,
    column_overrides: Vec<ColumnOverride>,
) -> Result<u32, AppError> {
    csv_execute_from_text(
        sqlite.inner(),
        connections.inner(),
        connection_id,
        csv_text,
        table_name,
        create_table,
        column_overrides,
    )
    .await
}

async fn sql_execute_from_text(
    app: tauri::AppHandle,
    sqlite: &sqlx::SqlitePool,
    connections: &Arc<ConnectionManager>,
    connection_id: String,
    content: String,
    stop_on_error: bool,
) -> Result<u32, AppError> {
    use tauri::Emitter;

    let profile_row = sqlx::query!(
        "SELECT read_only FROM connection_profiles WHERE id = ?",
        connection_id
    )
    .fetch_optional(sqlite)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    match profile_row {
        None => {
            return Err(AppError::new(
                "CONNECTION_NOT_FOUND",
                format!("No connection with id {connection_id}"),
            ))
        }
        Some(row) if row.read_only != 0 => {
            return Err(AppError::new(
                "READ_ONLY_VIOLATION",
                "This connection is in read-only mode — mutating statements are not allowed",
            ))
        }
        _ => {}
    }

    let statements = crate::lib_sql::split_sql_statements(&content);
    let total = statements.len() as u32;
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    let mut executed = 0u32;
    let mut errors = 0u32;

    for (i, stmt) in statements.iter().enumerate() {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }

        let result = match pool_ref.value() {
            RemotePool::MySql(pool, _) => sqlx::query(stmt).execute(pool).await.map(|_| ()),
            RemotePool::Postgres(pool) => sqlx::query(stmt).execute(pool).await.map(|_| ()),
            RemotePool::Sqlite(pool) => sqlx::query(stmt).execute(pool).await.map(|_| ()),
        };

        let error = result.err().map(|e| e.to_string());

        let progress = ImportProgress {
            current: i as u32 + 1,
            total: Some(total),
            statement: stmt.chars().take(200).collect::<String>(),
            error: error.clone(),
        };

        let _ = app.emit("import-sql-progress", &progress);

        if error.is_some() {
            errors += 1;
            if stop_on_error {
                break;
            }
        } else {
            executed += 1;
        }
    }

    let _ = errors;
    Ok(executed)
}

/// Execute all statements in a SQL file, emitting progress events.
#[tauri::command]
pub async fn import_sql_file(
    app: tauri::AppHandle,
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    file_path: String,
    stop_on_error: bool,
) -> Result<u32, AppError> {
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::new("IO_ERROR", format!("Cannot read {file_path}: {e}")))?;
    sql_execute_from_text(
        app,
        sqlite.inner(),
        connections.inner(),
        connection_id,
        content,
        stop_on_error,
    )
    .await
}

/// Execute SQL statements from a raw text string (e.g. clipboard content).
#[tauri::command]
pub async fn import_sql_text(
    app: tauri::AppHandle,
    sqlite: State<'_, sqlx::SqlitePool>,
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql_text: String,
    stop_on_error: bool,
) -> Result<u32, AppError> {
    sql_execute_from_text(
        app,
        sqlite.inner(),
        connections.inner(),
        connection_id,
        sql_text,
        stop_on_error,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_type_integers() {
        assert_eq!(infer_type(&["1", "2", "3"]), "integer");
    }

    #[test]
    fn infer_type_floats() {
        assert_eq!(infer_type(&["1.5", "2.0", "3.14"]), "float");
    }

    #[test]
    fn infer_type_dates() {
        assert_eq!(infer_type(&["2024-01-01", "2024-12-31"]), "date");
    }

    #[test]
    fn infer_type_text() {
        assert_eq!(infer_type(&["hello", "world"]), "text");
    }

    #[test]
    fn infer_type_empty_returns_text() {
        assert_eq!(infer_type(&["", ""]), "text");
        assert_eq!(infer_type(&[]), "text");
    }

    #[test]
    fn infer_type_mixed_falls_back_to_text() {
        assert_eq!(infer_type(&["1", "hello", "3"]), "text");
    }

    // ── Additional tests requested in audit ──────────────────────────────────

    #[test]
    fn infer_type_empty_string_returns_text() {
        // A slice containing only empty strings should return "text"
        // because there are no non-empty values to infer from.
        assert_eq!(infer_type(&[""]), "text");
    }

    #[test]
    fn infer_type_float_returns_float() {
        assert_eq!(infer_type(&["3.14"]), "float");
        assert_eq!(infer_type(&["0.0", "-1.5", "100.001"]), "float");
    }

    #[test]
    fn infer_type_iso_date_returns_date() {
        assert_eq!(infer_type(&["2024-06-11"]), "date");
        assert_eq!(infer_type(&["2000-01-01", "1999-12-31"]), "date");
    }

    #[test]
    fn preview_with_header_row_extracts_column_names() {
        // Build an in-memory CSV and run preview logic inline.
        let csv = "id,name,score\n1,Alice,9.5\n2,Bob,8.0\n";
        let mut reader = csv::Reader::from_reader(csv.as_bytes());

        let headers: Vec<String> = reader
            .headers()
            .unwrap()
            .iter()
            .map(|h| h.to_owned())
            .collect();

        assert_eq!(headers, vec!["id", "name", "score"]);

        // Collect column samples
        let mut column_samples: Vec<Vec<String>> = vec![Vec::new(); headers.len()];
        for result in reader.records().take(20) {
            let record = result.unwrap();
            let row: Vec<String> = record.iter().map(|f| f.to_owned()).collect();
            for (i, val) in row.iter().enumerate() {
                if i < column_samples.len() {
                    column_samples[i].push(val.clone());
                }
            }
        }

        // id column should infer integer
        let id_samples: Vec<&str> = column_samples[0].iter().map(|s| s.as_str()).collect();
        assert_eq!(infer_type(&id_samples), "integer");

        // score column should infer float
        let score_samples: Vec<&str> = column_samples[2].iter().map(|s| s.as_str()).collect();
        assert_eq!(infer_type(&score_samples), "float");
    }
}

/// Tauri commands for exporting query results to clipboard or file.
use std::io::Write as IoWrite;

use crate::error::AppError;

// ── Formatting helpers ────────────────────────────────────────────────────────

fn format_csv(rows: &[Vec<serde_json::Value>], columns: &[String]) -> String {
    let mut out = String::new();

    // Header row
    let header: Vec<String> = columns.iter().map(|c| csv_escape(c)).collect();
    out.push_str(&header.join(","));
    out.push('\n');

    for row in rows {
        let cells: Vec<String> = row.iter().map(csv_escape_value).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }

    out
}

fn format_json(rows: &[Vec<serde_json::Value>], columns: &[String]) -> String {
    let objects: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (i, col) in columns.iter().enumerate() {
                obj.insert(
                    col.clone(),
                    row.get(i).cloned().unwrap_or(serde_json::Value::Null),
                );
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    serde_json::to_string_pretty(&objects).unwrap_or_default()
}

fn format_sql_insert(
    rows: &[Vec<serde_json::Value>],
    columns: &[String],
    table_name: &str,
) -> String {
    if rows.is_empty() || columns.is_empty() {
        return String::new();
    }

    let cols = columns
        .iter()
        .map(|c| format!("`{}`", c.replace('`', "``")))
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = String::new();
    for row in rows {
        let vals: Vec<String> = row.iter().map(sql_value).collect();
        out.push_str(&format!(
            "INSERT INTO `{}` ({}) VALUES ({});\n",
            table_name.replace('`', "``"),
            cols,
            vals.join(", ")
        ));
    }
    out
}

fn format_sql_in_clause(rows: &[Vec<serde_json::Value>], columns: &[String]) -> String {
    if rows.is_empty() || columns.is_empty() {
        return String::new();
    }
    // Use the first column only.
    let vals: Vec<String> = rows
        .iter()
        .map(|row| sql_value(row.first().unwrap_or(&serde_json::Value::Null)))
        .collect();
    format!("({})", vals.join(", "))
}

fn format_tab_separated(rows: &[Vec<serde_json::Value>], columns: &[String]) -> String {
    let mut out = String::new();
    out.push_str(&columns.join("\t"));
    out.push('\n');
    for row in rows {
        let cells: Vec<String> = row.iter().map(json_value_to_string).collect();
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}

// ── Value converters ──────────────────────────────────────────────────────────

fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

fn csv_escape_value(v: &serde_json::Value) -> String {
    let s = json_value_to_string(v);
    csv_escape(&s)
}

fn sql_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_owned(),
        serde_json::Value::Bool(b) => {
            if *b {
                "1".to_owned()
            } else {
                "0".to_owned()
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        other => format!("'{}'", other.to_string().replace('\'', "''")),
    }
}

fn apply_format(
    rows: &[Vec<serde_json::Value>],
    columns: &[String],
    format: &str,
    table_name: Option<&str>,
) -> Result<String, AppError> {
    match format {
        "csv" => Ok(format_csv(rows, columns)),
        "json" => Ok(format_json(rows, columns)),
        "sql_insert" => {
            let table = table_name.unwrap_or("table_name");
            Ok(format_sql_insert(rows, columns, table))
        }
        "sql_in_clause" => Ok(format_sql_in_clause(rows, columns)),
        "tab_separated" => Ok(format_tab_separated(rows, columns)),
        other => Err(AppError::new(
            "EXPORT_ERROR",
            format!("Unknown format: {other}"),
        )),
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Export query results to the system clipboard.
#[tauri::command]
pub async fn export_result_to_clipboard(
    _app: tauri::AppHandle,
    rows: Vec<Vec<serde_json::Value>>,
    columns: Vec<String>,
    format: String,
    table_name: Option<String>,
) -> Result<(), AppError> {
    let content = apply_format(&rows, &columns, &format, table_name.as_deref())?;

    // Use arboard for clipboard access.
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| AppError::new("CLIPBOARD_ERROR", e.to_string()))?;
    clipboard
        .set_text(content)
        .map_err(|e| AppError::new("CLIPBOARD_ERROR", e.to_string()))?;

    Ok(())
}

/// Export query results to a file.
#[tauri::command]
pub async fn export_result_to_file(
    rows: Vec<Vec<serde_json::Value>>,
    columns: Vec<String>,
    format: String,
    file_path: String,
    table_name: Option<String>,
) -> Result<(), AppError> {
    let content = apply_format(&rows, &columns, &format, table_name.as_deref())?;

    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| AppError::new("IO_ERROR", format!("Cannot create {file_path}: {e}")))?;
    file.write_all(content.as_bytes())
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_rows() -> Vec<Vec<serde_json::Value>> {
        vec![
            vec![json!(1), json!("Alice"), json!(null)],
            vec![json!(2), json!("Bob \"Jr\""), json!(true)],
        ]
    }

    fn sample_cols() -> Vec<String> {
        vec!["id".into(), "name".into(), "active".into()]
    }

    #[test]
    fn csv_header_and_rows() {
        let out = format_csv(&sample_rows(), &sample_cols());
        assert!(out.starts_with("id,name,active\n"));
        assert!(out.contains("1,Alice,"));
    }

    #[test]
    fn csv_escapes_double_quotes() {
        let out = format_csv(&sample_rows(), &sample_cols());
        assert!(out.contains("\"Bob \"\"Jr\"\"\""));
    }

    #[test]
    fn json_produces_array_of_objects() {
        let out = format_json(&sample_rows(), &sample_cols());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["id"], json!(1));
    }

    #[test]
    fn sql_insert_format() {
        let out = format_sql_insert(&sample_rows(), &sample_cols(), "users");
        assert!(out.contains("INSERT INTO `users`"));
        assert!(out.contains("VALUES (1, 'Alice', NULL)"));
    }

    #[test]
    fn sql_in_clause_uses_first_column() {
        let out = format_sql_in_clause(&sample_rows(), &sample_cols());
        assert_eq!(out, "(1, 2)");
    }

    #[test]
    fn tab_separated_format() {
        let out = format_tab_separated(&sample_rows(), &sample_cols());
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "id\tname\tactive");
        assert_eq!(lines[1], "1\tAlice\t");
    }

    #[test]
    fn unknown_format_returns_error() {
        let result = apply_format(&[], &[], "xml", None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "EXPORT_ERROR");
    }

    #[test]
    fn sql_value_null() {
        assert_eq!(sql_value(&serde_json::Value::Null), "NULL");
    }

    #[test]
    fn sql_value_string_with_quote() {
        assert_eq!(sql_value(&json!("it's")), "'it''s'");
    }
}

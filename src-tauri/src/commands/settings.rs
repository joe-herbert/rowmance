/// Tauri commands for application settings.
/// Settings are stored as JSON values in the local SQLite `settings` table.
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tauri::State;

use crate::error::AppError;

/// Return all settings as a map of key → JSON value.
#[tauri::command]
pub async fn settings_get_all(
    sqlite: State<'_, SqlitePool>,
) -> Result<HashMap<String, Value>, AppError> {
    let rows = sqlx::query!("SELECT key, value FROM settings")
        .fetch_all(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let mut map = HashMap::new();
    for row in rows {
        if let (Some(key), Ok(parsed)) = (row.key, serde_json::from_str::<Value>(&row.value)) {
            map.insert(key, parsed);
        }
    }
    Ok(map)
}

/// Return a single setting value.
#[tauri::command]
pub async fn settings_get(
    sqlite: State<'_, SqlitePool>,
    key: String,
) -> Result<Option<Value>, AppError> {
    let row = sqlx::query!("SELECT value FROM settings WHERE key = ?", key)
        .fetch_optional(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    match row {
        Some(r) => Ok(serde_json::from_str(&r.value).ok()),
        None => Ok(None),
    }
}

/// Upsert a setting value.
#[tauri::command]
pub async fn settings_set(
    sqlite: State<'_, SqlitePool>,
    key: String,
    value: Value,
) -> Result<(), AppError> {
    let json_str = serde_json::to_string(&value)
        .map_err(|e| AppError::new("SERIALISATION_ERROR", e.to_string()))?;

    sqlx::query!(
        "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        key,
        json_str
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(())
}

/// Reset a setting to its default by deleting it from the table.
#[tauri::command]
pub async fn settings_reset(sqlite: State<'_, SqlitePool>, key: String) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM settings WHERE key = ?", key)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

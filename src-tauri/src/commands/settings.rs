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

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn get_all_returns_empty_map_initially() {
        let pool = setup_db().await;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn set_and_get_round_trip() {
        let pool = setup_db().await;
        let json_val = serde_json::to_string(&serde_json::json!("dark")).unwrap();

        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind("theme")
        .bind(&json_val)
        .execute(&pool)
        .await
        .unwrap();

        let value: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind("theme")
            .fetch_one(&pool)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&value).unwrap();
        assert_eq!(parsed, serde_json::json!("dark"));
    }

    #[tokio::test]
    async fn upsert_overwrites_existing_value() {
        let pool = setup_db().await;

        for val in [serde_json::json!(50), serde_json::json!(100)] {
            let json_str = serde_json::to_string(&val).unwrap();
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind("page_size")
            .bind(&json_str)
            .execute(&pool)
            .await
            .unwrap();
        }

        let value: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind("page_size")
            .fetch_one(&pool)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&value).unwrap();
        assert_eq!(parsed, serde_json::json!(100));
    }

    #[tokio::test]
    async fn reset_removes_the_key() {
        let pool = setup_db().await;
        let json_str = serde_json::to_string(&serde_json::json!("light")).unwrap();

        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?)")
            .bind("theme")
            .bind(&json_str)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind("theme")
            .execute(&pool)
            .await
            .unwrap();

        let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind("theme")
            .fetch_optional(&pool)
            .await
            .unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn get_all_returns_multiple_keys() {
        let pool = setup_db().await;

        for (k, v) in [("theme", "\"dark\""), ("page_size", "50")] {
            sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?)")
                .bind(k)
                .bind(v)
                .execute(&pool)
                .await
                .unwrap();
        }

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
}

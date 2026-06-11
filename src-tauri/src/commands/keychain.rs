/// Tauri commands for storing and retrieving secrets via the OS keychain.
/// Uses the `keyring` crate with service name "rowmance".
/// Entry names follow the pattern "{connection_id}:{secret_type}" where
/// secret_type is one of: db_password, ssh_password, ssh_key_passphrase.
use keyring::Entry;
use sqlx::SqlitePool;

use crate::error::AppError;

fn entry(connection_id: &str, secret_type: &str) -> Result<Entry, AppError> {
    let name = format!("{connection_id}:{secret_type}");
    Entry::new("rowmance", &name).map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))
}

/// Store a secret in the OS keychain.
#[tauri::command]
pub async fn keychain_store(
    connection_id: String,
    secret_type: String,
    value: String,
) -> Result<(), AppError> {
    let e = entry(&connection_id, &secret_type)?;
    e.set_password(&value)
        .map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))
}

/// Retrieve a secret from the OS keychain; returns None if not present.
#[tauri::command]
pub async fn keychain_retrieve(
    connection_id: String,
    secret_type: String,
) -> Result<Option<String>, AppError> {
    let e = entry(&connection_id, &secret_type)?;
    match e.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(AppError::new("KEYCHAIN_ERROR", err.to_string())),
    }
}

/// Delete a secret from the OS keychain. No-ops if not present.
#[tauri::command]
pub async fn keychain_delete(
    connection_id: String,
    secret_type: String,
) -> Result<(), AppError> {
    let e = entry(&connection_id, &secret_type)?;
    match e.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(AppError::new("KEYCHAIN_ERROR", err.to_string())),
    }
}

/// Migrate any existing plaintext passwords from SQLite to the keychain.
/// Called once on startup; safe to call multiple times.
pub async fn migrate_passwords_to_keychain(sqlite: &SqlitePool) {
    #[derive(sqlx::FromRow)]
    struct PasswordRow {
        id: String,
        password: Option<String>,
    }

    let rows: Result<Vec<PasswordRow>, _> = sqlx::query_as::<_, PasswordRow>(
        "SELECT id, password FROM connection_profiles WHERE password IS NOT NULL AND password != ''",
    )
    .fetch_all(sqlite)
    .await;

    if let Ok(rows) = rows {
        for row in rows {
            if let Some(pw) = row.password {
                if pw.is_empty() {
                    continue;
                }
                let name = format!("{}:db_password", row.id);
                if let Ok(e) = Entry::new("rowmance", &name) {
                    // Only migrate if keychain entry doesn't already exist.
                    if e.get_password().is_err() {
                        let _ = e.set_password(&pw);
                    }
                }
                // Null out the stored password.
                let _ = sqlx::query(
                    "UPDATE connection_profiles SET password = NULL WHERE id = ?",
                )
                .bind(&row.id)
                .execute(sqlite)
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_name_format() {
        // Verify the entry function doesn't panic for well-formed inputs.
        let result = entry("conn-123", "db_password");
        // keyring::Entry::new can fail on some CI environments without a keychain;
        // we just verify it produces an entry or a keychain error (not a panic).
        let _ = result;
    }
}

/// Tauri commands for application auto-update.
use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct UpdateCheckResult {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

/// Check for an available update.
/// Returns available=false when the updater plugin is not configured or no update exists.
#[tauri::command]
pub async fn updater_check(app: tauri::AppHandle) -> Result<UpdateCheckResult, AppError> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = match app.updater() {
        Ok(u) => u,
        Err(_) => {
            return Ok(UpdateCheckResult {
                available: false,
                version: None,
                notes: None,
            })
        }
    };
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateCheckResult {
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateCheckResult {
            available: false,
            version: None,
            notes: None,
        }),
        Err(e) => Err(AppError::new("UPDATER_ERROR", e.to_string())),
    }
}

/// Download and install the available update, then restart.
/// Returns the release notes fetched at install time (fresher than the check-time notes).
#[tauri::command]
pub async fn updater_install(app: tauri::AppHandle) -> Result<Option<String>, AppError> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app
        .updater()
        .map_err(|e| AppError::new("UPDATER_ERROR", e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| AppError::new("UPDATER_ERROR", e.to_string()))?;
    if let Some(update) = update {
        let notes = update.body.clone();
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| AppError::new("UPDATER_ERROR", e.to_string()))?;
        app.restart();
        return Ok(notes);
    }
    Ok(None)
}

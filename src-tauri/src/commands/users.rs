/// Tauri commands for user management.
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::ConnectionManager;
use crate::connections::types::DbUser;
use crate::error::AppError;

#[tauri::command]
pub async fn users_list(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<DbUser>, AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine.list_users().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn users_get_grants(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
) -> Result<Vec<String>, AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine
        .get_user_grants(&username, host.as_deref())
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn users_create(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
    password: String,
    is_superuser: bool,
    can_create_db: bool,
    can_create_role: bool,
) -> Result<(), AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine
        .create_user(
            &username,
            host.as_deref(),
            &password,
            is_superuser,
            can_create_db,
            can_create_role,
        )
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn users_drop(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
) -> Result<(), AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine
        .drop_user(&username, host.as_deref())
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn users_set_password(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
    password: String,
) -> Result<(), AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine
        .update_user_password(&username, host.as_deref(), &password)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn users_rename(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
    new_username: String,
    new_host: Option<String>,
) -> Result<(), AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine
        .rename_user(
            &username,
            host.as_deref(),
            &new_username,
            new_host.as_deref(),
        )
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn users_execute_grant(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
) -> Result<(), AppError> {
    let engine = connections
        .get_engine(&connection_id)
        .map_err(AppError::from)?;
    engine.execute_grant(&sql).await.map_err(AppError::from)
}

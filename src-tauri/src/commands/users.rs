/// Tauri commands for user management.
use serde::Serialize;
use sqlx::Row;
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct DbUser {
    pub username: String,
    pub host: Option<String>,
    #[serde(rename = "isSuperuser")]
    pub is_superuser: bool,
    #[serde(rename = "canCreateDb")]
    pub can_create_db: bool,
    #[serde(rename = "canCreateRole")]
    pub can_create_role: bool,
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
}

// ── MySQL helpers ─────────────────────────────────────────────────────────────

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn users_list(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<DbUser>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let rows = sqlx::query(
                "SELECT CAST(User AS CHAR) AS username, CAST(Host AS CHAR) AS host, \
                 Super_priv, Create_priv, \
                 IFNULL(account_locked, 'N') AS account_locked \
                 FROM mysql.user ORDER BY User, Host",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

            let users = rows
                .into_iter()
                .map(|row| {
                    let username: String = row.try_get("username").unwrap_or_default();
                    let host: Option<String> = row.try_get("host").ok();
                    let super_priv: Option<String> = row.try_get("Super_priv").ok();
                    let create_priv: Option<String> = row.try_get("Create_priv").ok();
                    let account_locked: Option<String> = row.try_get("account_locked").ok();
                    DbUser {
                        username,
                        host,
                        is_superuser: super_priv.as_deref() == Some("Y"),
                        can_create_db: create_priv.as_deref() == Some("Y"),
                        can_create_role: false,
                        is_locked: account_locked.as_deref() == Some("Y"),
                    }
                })
                .collect();
            Ok(users)
        }
        RemotePool::Postgres(pool) => {
            #[derive(sqlx::FromRow)]
            struct PgRoleRow {
                username: String,
                rolsuper: bool,
                rolcreatedb: bool,
                rolcreaterole: bool,
                is_locked: bool,
            }
            let rows = sqlx::query_as::<_, PgRoleRow>(
                "SELECT rolname AS username, rolsuper, rolcreatedb, rolcreaterole, \
                 NOT rolcanlogin AS is_locked \
                 FROM pg_roles WHERE rolcanlogin ORDER BY rolname",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

            let users = rows
                .into_iter()
                .map(|r| DbUser {
                    username: r.username,
                    host: None,
                    is_superuser: r.rolsuper,
                    can_create_db: r.rolcreatedb,
                    can_create_role: r.rolcreaterole,
                    is_locked: r.is_locked,
                })
                .collect();
            Ok(users)
        }
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
}

#[tauri::command]
pub async fn users_get_grants(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
) -> Result<Vec<String>, AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let h = host.as_deref().unwrap_or("%");
            let escaped_user = escape_sql_string(&username);
            let escaped_host = escape_sql_string(h);
            let sql = format!("SHOW GRANTS FOR '{escaped_user}'@'{escaped_host}'");
            let rows = sqlx::query(&sql)
                .fetch_all(pool)
                .await
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
            let grants = rows
                .into_iter()
                .map(|row| row.try_get::<String, _>(0).unwrap_or_default())
                .collect();
            Ok(grants)
        }
        RemotePool::Postgres(pool) => {
            // Query role attributes
            #[derive(sqlx::FromRow)]
            struct PgRoleAttrs {
                rolsuper: bool,
                rolcreatedb: bool,
                rolcreaterole: bool,
                rolbypassrls: bool,
            }
            let attrs = sqlx::query_as::<_, PgRoleAttrs>(
                "SELECT rolsuper, rolcreatedb, rolcreaterole, rolbypassrls \
                 FROM pg_roles WHERE rolname = $1",
            )
            .bind(&username)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

            let mut grants: Vec<String> = Vec::new();

            if let Some(a) = attrs {
                let super_str = if a.rolsuper {
                    "SUPERUSER"
                } else {
                    "NOSUPERUSER"
                };
                let createdb_str = if a.rolcreatedb {
                    "CREATEDB"
                } else {
                    "NOCREATEDB"
                };
                let createrole_str = if a.rolcreaterole {
                    "CREATEROLE"
                } else {
                    "NOCREATEROLE"
                };
                let bypassrls_str = if a.rolbypassrls {
                    "BYPASSRLS"
                } else {
                    "NOBYPASSRLS"
                };
                grants.push(format!(
                    "ALTER ROLE \"{username}\" {super_str} {createdb_str} {createrole_str} {bypassrls_str};"
                ));
            }

            // Query role memberships
            #[derive(sqlx::FromRow)]
            struct PgMember {
                member_of: String,
            }
            let memberships = sqlx::query_as::<_, PgMember>(
                "SELECT r.rolname AS member_of \
                 FROM pg_auth_members m \
                 JOIN pg_roles r ON r.oid = m.roleid \
                 JOIN pg_roles u ON u.oid = m.member \
                 WHERE u.rolname = $1",
            )
            .bind(&username)
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

            for m in memberships {
                grants.push(format!("GRANT \"{}\" TO \"{username}\";", m.member_of));
            }

            Ok(grants)
        }
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
}

#[tauri::command]
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
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let h = host.as_deref().unwrap_or("%");
            let eu = escape_sql_string(&username);
            let eh = escape_sql_string(h);
            let ep = escape_sql_string(&password);
            let sql = format!("CREATE USER '{eu}'@'{eh}' IDENTIFIED BY '{ep}'");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Postgres(pool) => {
            let ep = escape_sql_string(&password);
            let mut parts = vec![format!("CREATE USER \"{username}\" WITH PASSWORD '{ep}'")];
            if is_superuser {
                parts.push("SUPERUSER".to_string());
            }
            if can_create_db {
                parts.push("CREATEDB".to_string());
            }
            if can_create_role {
                parts.push("CREATEROLE".to_string());
            }
            let sql = parts.join(" ");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
}

#[tauri::command]
pub async fn users_drop(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
) -> Result<(), AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let h = host.as_deref().unwrap_or("%");
            let eu = escape_sql_string(&username);
            let eh = escape_sql_string(h);
            let sql = format!("DROP USER '{eu}'@'{eh}'");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Postgres(pool) => {
            let sql = format!("DROP USER \"{username}\"");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
}

#[tauri::command]
pub async fn users_set_password(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    username: String,
    host: Option<String>,
    password: String,
) -> Result<(), AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let h = host.as_deref().unwrap_or("%");
            let eu = escape_sql_string(&username);
            let eh = escape_sql_string(h);
            let ep = escape_sql_string(&password);
            let sql = format!("ALTER USER '{eu}'@'{eh}' IDENTIFIED BY '{ep}'");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Postgres(pool) => {
            let ep = escape_sql_string(&password);
            let sql = format!("ALTER USER \"{username}\" WITH PASSWORD '{ep}'");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
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
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => {
            let h = host.as_deref().unwrap_or("%");
            let nh = new_host.as_deref().unwrap_or("%");
            let eu = escape_sql_string(&username);
            let eh = escape_sql_string(h);
            let enu = escape_sql_string(&new_username);
            let enh = escape_sql_string(nh);
            let sql = format!("RENAME USER '{eu}'@'{eh}' TO '{enu}'@'{enh}'");
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|e| AppError::new("DB_ERROR", e.to_string()))
        }
        RemotePool::Postgres(pool) => {
            if username != new_username {
                let sql = format!("ALTER ROLE \"{username}\" RENAME TO \"{new_username}\"");
                sqlx::query(&sql)
                    .execute(pool)
                    .await
                    .map(|_| ())
                    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
            }
            Ok(())
        }
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
}

#[tauri::command]
pub async fn users_execute_grant(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    sql: String,
) -> Result<(), AppError> {
    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;
    match pool_ref.value() {
        RemotePool::MySql(pool) => sqlx::query(&sql)
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| AppError::new("DB_ERROR", e.to_string())),
        RemotePool::Postgres(pool) => sqlx::query(&sql)
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| AppError::new("DB_ERROR", e.to_string())),
        RemotePool::Sqlite(_) => Err(AppError::new(
            "UNSUPPORTED",
            "User management is not supported for SQLite connections",
        )),
    }
}

/// Editor session management.
///
/// A session pins one dedicated connection per query-editor tab, keyed by the
/// editor's UUID. Unlike transactions there are no BEGIN/COMMIT semantics —
/// the connection is held purely so that session-scoped state (e.g. MySQL
/// user-defined variables) survives across separate query executions.
use dashmap::DashMap;
use sqlx::ConnectOptions;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;
use crate::transactions::HeldConnection;

pub struct SessionManager {
    active: DashMap<String, Arc<Mutex<HeldConnection>>>,
}

impl SessionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active: DashMap::new(),
        })
    }

    pub fn get(&self, session_id: &str) -> Option<Arc<Mutex<HeldConnection>>> {
        self.active.get(session_id).map(|r| Arc::clone(&*r))
    }

    pub fn remove(&self, session_id: &str) {
        self.active.remove(session_id);
    }
}

/// Acquire a dedicated connection for the given editor session.
/// If a session is already active for this session_id it is released first,
/// so calling this again on database-change or reconnect is safe.
#[tauri::command]
pub async fn session_acquire(
    connections: State<'_, Arc<ConnectionManager>>,
    sessions: State<'_, Arc<SessionManager>>,
    connection_id: String,
    session_id: String,
) -> Result<(), AppError> {
    sessions.remove(&session_id);

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    let held = match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            // Direct connection (not a pool slot) so no after_release hook resets
            // the session state between queries. autocommit stays at the default (1).
            let conn = (*pool.connect_options())
                .clone()
                .connect()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            HeldConnection::MySql(conn)
        }
        RemotePool::Postgres(pool) => {
            let conn = pool
                .acquire()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            HeldConnection::Postgres(conn)
        }
        RemotePool::Sqlite(pool) => {
            let conn = pool
                .acquire()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            HeldConnection::Sqlite(conn)
        }
        RemotePool::SqlServer(pool, _) => {
            let conn = pool
                .get_owned()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            HeldConnection::SqlServer(conn)
        }
    };

    sessions
        .active
        .insert(session_id, Arc::new(Mutex::new(held)));
    Ok(())
}

/// Release the dedicated connection for the given editor session.
#[tauri::command]
pub async fn session_release(
    sessions: State<'_, Arc<SessionManager>>,
    session_id: String,
) -> Result<(), AppError> {
    sessions.remove(&session_id);
    Ok(())
}

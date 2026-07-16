/// Editor session management.
///
/// A session pins one dedicated connection per query-editor tab, keyed by the
/// editor's UUID. Unlike transactions there are no BEGIN/COMMIT semantics —
/// the connection is held purely so that session-scoped state (e.g. MySQL
/// user-defined variables) survives across separate query executions.
use dashmap::DashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::connections::engine::EngineTransaction;
use crate::connections::pool_manager::ConnectionManager;
use crate::error::AppError;

pub struct SessionManager {
    active: DashMap<String, Arc<Mutex<Box<dyn EngineTransaction>>>>,
}

impl SessionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active: DashMap::new(),
        })
    }

    pub fn get(&self, session_id: &str) -> Option<Arc<Mutex<Box<dyn EngineTransaction>>>> {
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

    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    let session = engine.begin_session().await.map_err(AppError::from)?;

    sessions
        .active
        .insert(session_id, Arc::new(Mutex::new(session)));
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

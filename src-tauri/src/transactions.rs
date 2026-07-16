/// Transaction management for the query editor and table browser.
///
/// When the user clicks "Begin Transaction", a single connection is checked
/// out and stored here. Every subsequent query command for that connection_id
/// routes through this held connection until the user commits or rolls back.
use dashmap::DashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::connections::engine::EngineTransaction;
use crate::connections::pool_manager::ConnectionManager;
use crate::error::AppError;

pub struct TransactionManager {
    active: DashMap<String, Arc<Mutex<Box<dyn EngineTransaction>>>>,
}

impl TransactionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active: DashMap::new(),
        })
    }

    /// Return the held connection for `connection_id`, if a transaction is active.
    pub fn get(&self, connection_id: &str) -> Option<Arc<Mutex<Box<dyn EngineTransaction>>>> {
        self.active.get(connection_id).map(|r| Arc::clone(&*r))
    }

    pub fn is_active(&self, connection_id: &str) -> bool {
        self.active.contains_key(connection_id)
    }

    /// Remove the held connection, returning it to the pool.
    pub fn remove(&self, connection_id: &str) {
        self.active.remove(connection_id);
    }
}

#[tauri::command]
pub async fn transaction_begin(
    connections: State<'_, Arc<ConnectionManager>>,
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
    database: Option<String>,
) -> Result<(), AppError> {
    if transactions.is_active(&connection_id) {
        return Err(AppError::new(
            "TRANSACTION_ACTIVE",
            "A transaction is already active for this connection",
        ));
    }

    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    let tx = engine
        .begin_transaction(database.as_deref())
        .await
        .map_err(AppError::from)?;

    transactions
        .active
        .insert(connection_id, Arc::new(Mutex::new(tx)));
    Ok(())
}

#[tauri::command]
pub async fn transaction_commit(
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
) -> Result<(), AppError> {
    let tx = transactions.get(&connection_id).ok_or_else(|| {
        AppError::new(
            "TRANSACTION_NOT_ACTIVE",
            "No active transaction for this connection",
        )
    })?;

    let mut guard = tx.lock().await;
    guard.commit().await.map_err(AppError::from)?;
    drop(guard);
    transactions.remove(&connection_id);
    Ok(())
}

#[tauri::command]
pub async fn transaction_rollback(
    transactions: State<'_, Arc<TransactionManager>>,
    connection_id: String,
) -> Result<(), AppError> {
    let tx = transactions.get(&connection_id).ok_or_else(|| {
        AppError::new(
            "TRANSACTION_NOT_ACTIVE",
            "No active transaction for this connection",
        )
    })?;

    let mut guard = tx.lock().await;
    guard.rollback().await.map_err(AppError::from)?;
    drop(guard);
    transactions.remove(&connection_id);
    Ok(())
}

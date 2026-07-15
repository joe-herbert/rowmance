/// Transaction management for the query editor and table browser.
///
/// When the user clicks "Begin Transaction", a single connection is checked
/// out and stored here. Every subsequent query command for that connection_id
/// routes through this held connection until the user commits or rolls back.
use dashmap::DashMap;
use sqlx::ConnectOptions;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::connections::pool_manager::{ConnectionManager, RemotePool};
use crate::error::AppError;

/// Type alias for an owned SQL Server pooled connection with a 'static lifetime.
pub type SqlServerOwnedConn =
    bb8::PooledConnection<'static, bb8_tiberius::ConnectionManager>;

pub enum HeldConnection {
    // MySQL uses a direct MySqlConnection (not PoolConnection) so we can open
    // it with the correct database already set in the connect options, avoiding
    // the need for USE statements (which MySQL error 1295 rejects as prepared
    // statements).
    MySql(sqlx::mysql::MySqlConnection),
    Postgres(sqlx::pool::PoolConnection<sqlx::Postgres>),
    Sqlite(sqlx::pool::PoolConnection<sqlx::Sqlite>),
    SqlServer(SqlServerOwnedConn),
}

pub struct TransactionManager {
    active: DashMap<String, Arc<Mutex<HeldConnection>>>,
}

impl TransactionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active: DashMap::new(),
        })
    }

    /// Return the held connection for `connection_id`, if a transaction is active.
    pub fn get(&self, connection_id: &str) -> Option<Arc<Mutex<HeldConnection>>> {
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

    let pool_ref = connections.get(&connection_id).map_err(AppError::from)?;

    let held = match pool_ref.value() {
        RemotePool::MySql(pool, _) => {
            // Open a direct connection with the target database set in options.
            // This avoids needing to issue USE later, which MySQL rejects as a
            // prepared statement (error 1295).
            let opts = if let Some(db) = &database {
                (*pool.connect_options()).clone().database(db)
            } else {
                (*pool.connect_options()).clone()
            };
            let mut conn = opts
                .connect()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            // MySQL error 1295: BEGIN and START TRANSACTION are not supported in
            // the prepared statement protocol. SET autocommit = 0 starts an
            // implicit transaction and is fully supported as a prepared statement.
            sqlx::query("SET autocommit = 0")
                .execute(&mut conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            HeldConnection::MySql(conn)
        }
        RemotePool::Postgres(pool) => {
            let mut conn = pool
                .acquire()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            sqlx::query("BEGIN")
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            HeldConnection::Postgres(conn)
        }
        RemotePool::Sqlite(pool) => {
            let mut conn = pool
                .acquire()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            sqlx::query("BEGIN")
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            HeldConnection::Sqlite(conn)
        }
        RemotePool::SqlServer(pool, _) => {
            let mut conn = pool
                .get_owned()
                .await
                .map_err(|e| AppError::new("POOL_ERROR", e.to_string()))?;
            crate::connections::sqlserver::exec_simple(&mut *conn, "BEGIN TRANSACTION")
                .await
                .map_err(AppError::from)?;
            HeldConnection::SqlServer(conn)
        }
    };

    transactions
        .active
        .insert(connection_id, Arc::new(Mutex::new(held)));
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
    match &mut *guard {
        HeldConnection::MySql(conn) => {
            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            // Restore autocommit so the connection is clean if it were reused.
            sqlx::query("SET autocommit = 1")
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        HeldConnection::Postgres(conn) => {
            sqlx::query("COMMIT")
                .execute(&mut **conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        HeldConnection::Sqlite(conn) => {
            sqlx::query("COMMIT")
                .execute(&mut **conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        HeldConnection::SqlServer(conn) => {
            crate::connections::sqlserver::exec_simple(&mut **conn, "COMMIT")
                .await
                .map_err(AppError::from)?;
        }
    };
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
    match &mut *guard {
        HeldConnection::MySql(conn) => {
            sqlx::query("ROLLBACK")
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
            sqlx::query("SET autocommit = 1")
                .execute(&mut *conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        HeldConnection::Postgres(conn) => {
            sqlx::query("ROLLBACK")
                .execute(&mut **conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        HeldConnection::Sqlite(conn) => {
            sqlx::query("ROLLBACK")
                .execute(&mut **conn)
                .await
                .map_err(|e| AppError::new("QUERY_ERROR", e.to_string()))?;
        }
        HeldConnection::SqlServer(conn) => {
            crate::connections::sqlserver::exec_simple(&mut **conn, "ROLLBACK")
                .await
                .map_err(AppError::from)?;
        }
    };
    drop(guard);
    transactions.remove(&connection_id);
    Ok(())
}

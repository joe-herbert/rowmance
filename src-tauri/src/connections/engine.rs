/// The `DatabaseEngine` trait — the single contract every database driver must implement.
///
/// Each engine is a struct wrapping its connection pool. All engine-specific code lives
/// inside the engine's impl; command handlers call trait methods and stay engine-agnostic.
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

use crate::connections::types::{
    BulkColumnRow, ColumnInfo, DbUser, ErdGraph, EngineQueryResult, ExplainResult, ForeignKeyInfo,
    IndexInfo, RowChange, RowDelete, TableInfo,
};
use crate::error::RowmanceError;

#[async_trait]
pub trait DatabaseEngine: Send + Sync {
    // ── Dialect helpers (synchronous) ─────────────────────────────────────────
    /// Quote an identifier (table name, column name, etc.) for this engine.
    fn quote(&self, ident: &str) -> String;

    /// Return true if this engine uses TRUE/FALSE boolean literals rather than 1/0.
    fn boolean_literals(&self) -> bool { false }

    /// Return the parameter placeholder for the nth bind position (1-indexed).
    /// MySQL/SQLite: `?`, PostgreSQL: `$1`, `$2`, …, SQL Server: `@P1`, `@P2`, …
    fn placeholder(&self, n: usize) -> String;

    /// Wrap a SELECT statement with engine-appropriate pagination syntax.
    fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String;

    /// Return true if this SQL string is expected to return rows.
    fn parse_returns_rows(&self, sql: &str) -> bool;

    // ── Schema introspection ──────────────────────────────────────────────────
    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError>;
    async fn list_tables(&self, database: &str) -> Result<Vec<TableInfo>, RowmanceError>;
    async fn list_columns(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, RowmanceError>;
    async fn list_all_columns(
        &self,
        database: &str,
    ) -> Result<Vec<BulkColumnRow>, RowmanceError>;
    async fn list_indexes(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<IndexInfo>, RowmanceError>;
    async fn list_foreign_keys(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, RowmanceError>;
    async fn count_table(&self, database: &str, table: &str) -> Result<i64, RowmanceError>;
    async fn get_ddl(&self, database: &str, table: &str) -> Result<String, RowmanceError>;

    // ── Query execution ───────────────────────────────────────────────────────
    /// Execute a SQL statement, switching to `database` context first if provided.
    /// Returns the result rows/columns for SELECT, or affected_rows for DML.
    async fn execute(
        &self,
        sql: &str,
        database: Option<&str>,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError>;

    /// Execute a DDL statement (CREATE, ALTER, DROP). Uses text-batch protocol
    /// where required (SQL Server) so transaction control words are not rejected.
    async fn execute_ddl(&self, sql: &str) -> Result<(), RowmanceError>;

    /// Count the total rows a SELECT would return, ignoring pagination.
    /// Returns None if counting is not practical (e.g., non-SELECT statements).
    async fn count_query_rows(&self, sql: &str, database: Option<&str>) -> Option<i64>;

    // ── Row mutations ─────────────────────────────────────────────────────────
    /// Apply a batch of UPDATEs, INSERTs, and DELETEs atomically.
    async fn apply_changes(
        &self,
        database: &str,
        table: &str,
        updates: &[RowChange],
        inserts: &[std::collections::HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError>;

    // ── Connection lifecycle ──────────────────────────────────────────────────
    async fn ping(&self) -> bool;

    // ── Transactions ──────────────────────────────────────────────────────────
    /// Check out a single connection, begin a transaction, and return it.
    async fn begin_transaction(
        &self,
        database: Option<&str>,
    ) -> Result<Box<dyn EngineTransaction>, RowmanceError>;

    /// Check out a single connection WITHOUT starting a transaction (for editor sessions).
    async fn begin_session(&self) -> Result<Box<dyn EngineTransaction>, RowmanceError>;

    // ── Query analysis ────────────────────────────────────────────────────────
    /// Run EXPLAIN (or equivalent) and return the plan as JSON + dialect tag.
    async fn explain(
        &self,
        sql: &str,
        database: Option<&str>,
    ) -> Result<ExplainResult, RowmanceError>;

    // ── Entity-relationship diagram ───────────────────────────────────────────
    /// Build the full entity-relationship graph for a database/schema.
    async fn get_erd_graph(&self, database: &str) -> Result<ErdGraph, RowmanceError>;

    // ── User management ───────────────────────────────────────────────────────
    async fn list_users(&self) -> Result<Vec<DbUser>, RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }
    async fn get_user_grants(
        &self,
        _username: &str,
        _host: Option<&str>,
    ) -> Result<Vec<String>, RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }
    async fn create_user(
        &self,
        _username: &str,
        _host: Option<&str>,
        _password: &str,
        _is_superuser: bool,
        _can_create_db: bool,
        _can_create_role: bool,
    ) -> Result<(), RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }
    async fn drop_user(
        &self,
        _username: &str,
        _host: Option<&str>,
    ) -> Result<(), RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }
    async fn update_user_password(
        &self,
        _username: &str,
        _host: Option<&str>,
        _password: &str,
    ) -> Result<(), RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }
    async fn rename_user(
        &self,
        _username: &str,
        _host: Option<&str>,
        _new_username: &str,
        _new_host: Option<&str>,
    ) -> Result<(), RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }
    async fn execute_grant(
        &self,
        _sql: &str,
    ) -> Result<(), RowmanceError> {
        Err(RowmanceError::ConnectionNotFound(
            "User management is not supported for this connection type".to_string(),
        ))
    }

    // ── Import ────────────────────────────────────────────────────────────────
    async fn import_csv(
        &self,
        database: &str,
        table: &str,
        headers: &[String],
        rows: &[Vec<String>],
        create_table: bool,
    ) -> Result<u64, RowmanceError>;
}

/// A held connection pool for one database engine. Returned by `engines::connect_for_db_type`.
/// Wraps the pool handle and provides the three operations that `ConnectionManager` needs.
/// Engine files implement this trait so `pool_manager.rs` never matches on engine type.
#[async_trait]
pub trait PoolAdapter: Send + Sync + fmt::Debug {
    /// Gracefully close all connections in the pool.
    async fn disconnect(&self);
    /// Return true if a lightweight probe query succeeds.
    async fn ping(&self) -> bool;
    /// Construct an engine trait object backed by this pool.
    fn get_engine(&self) -> Arc<dyn DatabaseEngine>;
}

/// A held database connection with an open transaction. Returned by
/// `DatabaseEngine::begin_transaction()` and stored in `TransactionManager`.
#[async_trait]
pub trait EngineTransaction: Send {
    async fn execute(
        &mut self,
        sql: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<EngineQueryResult, RowmanceError>;

    async fn apply_changes(
        &mut self,
        database: &str,
        table: &str,
        updates: &[RowChange],
        inserts: &[std::collections::HashMap<String, serde_json::Value>],
        deletes: &[RowDelete],
    ) -> Result<(u64, u64, u64), RowmanceError>;

    async fn execute_raw(&mut self, sql: &str) -> Result<(), RowmanceError>;

    async fn commit(&mut self) -> Result<(), RowmanceError>;
    async fn rollback(&mut self) -> Result<(), RowmanceError>;

    /// Switch schema/database context within the held connection.
    async fn switch_context(&mut self, database: &str) -> Result<(), RowmanceError>;
}

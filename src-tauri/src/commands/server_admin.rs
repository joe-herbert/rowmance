/// Tauri commands for server administration.

/// Returns true if `session_id` is a valid Oracle "sid,serial#" format:
/// digits, a single comma, then more digits (e.g. "42,1234").
pub(crate) fn is_valid_oracle_session_id(session_id: &str) -> bool {
    let parts: Vec<&str> = session_id.splitn(2, ',').collect();
    parts.len() == 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && !session_id[parts[0].len() + 1..].contains(',')
}

use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::ConnectionManager;
use crate::connections::types::{
    LockInfo, ProcessInfo, ScheduledJob, ServerAdminCapabilityFlags, ServerStatus, ServerVariable,
    VacuumInfo, VarScope,
};
use crate::error::AppError;

#[tauri::command]
pub async fn server_admin_get_capabilities(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<ServerAdminCapabilityFlags, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.probe_server_admin_capabilities().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_list_processes(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<ProcessInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_processes().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_kill_session(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    session_id: String,
) -> Result<(), AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.kill_session(&session_id).await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_cancel_session(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    pid: String,
) -> Result<(), AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.cancel_session(&pid).await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_get_status(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<ServerStatus, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.get_server_status().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_list_variables(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<ServerVariable>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_variables().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_set_variable(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    name: String,
    value: String,
    scope: VarScope,
) -> Result<(), AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.set_variable(&name, &value, scope).await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_list_locks(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<LockInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_locks().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_list_scheduled_jobs(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<ScheduledJob>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.list_scheduled_jobs().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_get_innodb_status(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<String, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.get_innodb_status().await.map_err(AppError::from)
}

#[tauri::command]
pub async fn server_admin_get_vacuum_status(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
) -> Result<Vec<VacuumInfo>, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.get_vacuum_status().await.map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::engine::DatabaseEngine;
    use crate::connections::types::{
        BulkColumnRow, CapabilityStatus, ColumnInfo, ErdGraph, EngineQueryResult,
        ExplainResult, ForeignKeyInfo, IndexInfo, RowChange, RowDelete, TableInfo,
    };
    use crate::error::RowmanceError;
    use async_trait::async_trait;

    // ── Oracle session-ID validation ──────────────────────────────────────────

    #[test]
    fn valid_oracle_session_id_accepted() {
        assert!(is_valid_oracle_session_id("42,1234"));
        assert!(is_valid_oracle_session_id("1,1"));
        assert!(is_valid_oracle_session_id("999,99999"));
    }

    #[test]
    fn oracle_session_id_missing_comma_rejected() {
        assert!(!is_valid_oracle_session_id("42"));
        assert!(!is_valid_oracle_session_id("421234"));
    }

    #[test]
    fn oracle_session_id_multiple_commas_rejected() {
        assert!(!is_valid_oracle_session_id("42,12,34"));
        assert!(!is_valid_oracle_session_id(",,,"));
    }

    #[test]
    fn oracle_session_id_non_numeric_rejected() {
        assert!(!is_valid_oracle_session_id("abc,123"));
        assert!(!is_valid_oracle_session_id("42,1a34"));
        assert!(!is_valid_oracle_session_id("' OR 1=1,0"));
        assert!(!is_valid_oracle_session_id(""));
    }

    #[test]
    fn oracle_session_id_trailing_comma_rejected() {
        assert!(!is_valid_oracle_session_id("42,"));
    }

    #[test]
    fn oracle_session_id_leading_comma_rejected() {
        assert!(!is_valid_oracle_session_id(",1234"));
    }

    // ── Default trait implementations return NotSupported / ConnectionNotFound ─

    /// Minimal engine stub that relies entirely on the trait's default implementations
    /// for all server-admin methods.
    struct NoOpEngine;

    #[async_trait]
    impl DatabaseEngine for NoOpEngine {
        fn quote(&self, ident: &str) -> String { format!("\"{ident}\"") }
        fn placeholder(&self, _n: usize) -> String { "?".to_string() }
        fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String {
            format!("{sql} LIMIT {limit} OFFSET {offset}")
        }
        fn parse_returns_rows(&self, _sql: &str) -> bool { false }

        async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> { Ok(vec![]) }
        async fn list_tables(&self, _db: &str, _inst: Option<&str>) -> Result<Vec<TableInfo>, RowmanceError> { Ok(vec![]) }
        async fn list_columns(&self, _db: &str, _t: &str, _inst: Option<&str>) -> Result<Vec<ColumnInfo>, RowmanceError> { Ok(vec![]) }
        async fn list_all_columns(&self, _db: &str, _inst: Option<&str>) -> Result<Vec<BulkColumnRow>, RowmanceError> { Ok(vec![]) }
        async fn list_indexes(&self, _db: &str, _t: &str, _inst: Option<&str>) -> Result<Vec<IndexInfo>, RowmanceError> { Ok(vec![]) }
        async fn list_foreign_keys(&self, _db: &str, _t: &str, _inst: Option<&str>) -> Result<Vec<ForeignKeyInfo>, RowmanceError> { Ok(vec![]) }
        async fn count_table(&self, _db: &str, _t: &str, _inst: Option<&str>) -> Result<i64, RowmanceError> { Ok(0) }
        async fn get_ddl(&self, _db: &str, _t: &str, _inst: Option<&str>) -> Result<String, RowmanceError> {
            Err(RowmanceError::ConnectionNotFound("no ddl".into()))
        }
        async fn execute(&self, _sql: &str, _db: Option<&str>, _inst: Option<&str>, _page_size: u32, _offset: u32) -> Result<EngineQueryResult, RowmanceError> {
            Ok(EngineQueryResult { columns: vec![], rows: vec![], affected_rows: None })
        }
        async fn execute_ddl(&self, _sql: &str) -> Result<(), RowmanceError> { Ok(()) }
        async fn count_query_rows(&self, _sql: &str, _db: Option<&str>, _inst: Option<&str>) -> Option<i64> { None }
        async fn apply_changes(&self, _db: &str, _t: &str, _inst: Option<&str>, _updates: &[RowChange], _inserts: &[std::collections::HashMap<String, serde_json::Value>], _deletes: &[RowDelete]) -> Result<(u64, u64, u64), RowmanceError> {
            Ok((0, 0, 0))
        }
        async fn ping(&self) -> bool { true }
        async fn begin_transaction(&self, _db: Option<&str>) -> Result<Box<dyn crate::connections::engine::EngineTransaction>, RowmanceError> {
            Err(RowmanceError::ConnectionNotFound("no tx".into()))
        }
        async fn begin_session(&self) -> Result<Box<dyn crate::connections::engine::EngineTransaction>, RowmanceError> {
            Err(RowmanceError::ConnectionNotFound("no session".into()))
        }
        async fn explain(&self, _sql: &str, _db: Option<&str>, _inst: Option<&str>) -> Result<ExplainResult, RowmanceError> {
            Err(RowmanceError::ConnectionNotFound("no explain".into()))
        }
        async fn get_erd_graph(&self, _db: &str, _inst: Option<&str>) -> Result<ErdGraph, RowmanceError> {
            Err(RowmanceError::ConnectionNotFound("no erd".into()))
        }
        async fn import_csv(&self, _db: &str, _t: &str, _inst: Option<&str>, _headers: &[String], _rows: &[Vec<String>], _create: bool) -> Result<u64, RowmanceError> {
            Ok(0)
        }
    }

    #[tokio::test]
    async fn default_probe_capabilities_returns_all_not_supported() {
        let engine = NoOpEngine;
        let caps = engine.probe_server_admin_capabilities().await.unwrap();
        assert_eq!(caps.process_list, CapabilityStatus::NotSupported);
        assert_eq!(caps.kill_session, CapabilityStatus::NotSupported);
        assert_eq!(caps.cancel_session, CapabilityStatus::NotSupported);
        assert_eq!(caps.server_status, CapabilityStatus::NotSupported);
        assert_eq!(caps.variables, CapabilityStatus::NotSupported);
        assert_eq!(caps.set_variable, CapabilityStatus::NotSupported);
        assert_eq!(caps.scheduled_jobs, CapabilityStatus::NotSupported);
        assert_eq!(caps.locks, CapabilityStatus::NotSupported);
        assert_eq!(caps.innodb_status, CapabilityStatus::NotSupported);
        assert_eq!(caps.vacuum_status, CapabilityStatus::NotSupported);
    }

    #[tokio::test]
    async fn default_list_processes_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.list_processes().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_kill_session_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.kill_session("42").await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_cancel_session_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.cancel_session("1234").await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_get_server_status_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.get_server_status().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_list_variables_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.list_variables().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_set_variable_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.set_variable("max_connections", "200", VarScope::Global).await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_list_locks_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.list_locks().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_list_scheduled_jobs_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.list_scheduled_jobs().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_get_innodb_status_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.get_innodb_status().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }

    #[tokio::test]
    async fn default_get_vacuum_status_returns_connection_not_found() {
        let engine = NoOpEngine;
        assert!(matches!(engine.get_vacuum_status().await.unwrap_err(), RowmanceError::ConnectionNotFound(_)));
    }
}

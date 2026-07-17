pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod sqlserver;

use crate::connections::types::DialectInfo;
use crate::connections::engine::PoolAdapter;
use crate::error::RowmanceError;

/// Return dialect metadata for the given db_type string.
/// Derived from `all_known_dialects()` so there is no separate list to maintain.
pub fn dialect_for_db_type(db_type: &str) -> DialectInfo {
    all_known_dialects()
        .into_iter()
        .find(|(k, _)| k == db_type)
        .map(|(_, v)| v)
        .unwrap_or_else(|| default_dialect(db_type))
}

/// Return dialect metadata for every engine known to this build.
/// The frontend uses this to populate the engine type dropdown.
pub fn all_known_dialects() -> Vec<(String, DialectInfo)> {
    vec![
        ("mysql".to_owned(),     mysql::dialect_info("mysql").unwrap()),
        ("mariadb".to_owned(),   mysql::dialect_info("mariadb").unwrap()),
        ("postgres".to_owned(),  postgres::dialect_info("postgres").unwrap()),
        ("sqlite".to_owned(),    sqlite::dialect_info("sqlite").unwrap()),
        ("sqlserver".to_owned(), sqlserver::dialect_info("sqlserver").unwrap()),
    ]
}

/// Open a connection pool for the given db_type.
/// All engine-specific code lives in the engine file; only this dispatch
/// needs updating when adding a new engine.
pub async fn connect_for_db_type(
    db_type: &str,
    host: &str,
    port: u16,
    database: &str,
    username: &str,
    password: &str,
    pool_max: u32,
    ssl_enabled: bool,
    ssl_ca_path: Option<&str>,
    ssl_cert_path: Option<&str>,
    ssl_key_path: Option<&str>,
    read_only: bool,
) -> Result<Box<dyn PoolAdapter>, RowmanceError> {
    match db_type {
        "mysql" | "mariadb" => {
            mysql::create_pool(host, port, database, username, password, pool_max, ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path, read_only).await
        }
        "postgres" => {
            postgres::create_pool(host, port, database, username, password, pool_max, ssl_enabled, ssl_ca_path, ssl_cert_path, ssl_key_path, read_only).await
        }
        "sqlite" => {
            sqlite::create_pool(host, pool_max, read_only).await
        }
        "sqlserver" => {
            sqlserver::create_pool(host, port, database, username, password, pool_max, ssl_enabled, ssl_ca_path, read_only).await
        }
        other => Err(RowmanceError::ConnectionNotFound(format!("Unknown db_type: {other}")))
    }
}

fn default_dialect(db_type: &str) -> DialectInfo {
    DialectInfo {
        identifier_open: "\"".into(),
        identifier_close: "\"".into(),
        identifier_escape: "\"\"".into(),
        uses_schema: true,
        db_label: "Database".into(),
        has_instance_databases: false,
        select_top: false,
        boolean_literals: false,
        uses_ilike: false,
        cast_to_text: "CAST({col} AS TEXT)".into(),
        supports_user_management: false,
        host_based_users: false,
        supports_roles: false,
        detects_sql_variables: false,
        warns_tx_database_mismatch: false,
        display_name: db_type.to_string(),
        default_column_type: "TEXT".into(),
        common_column_types: vec!["TEXT".into(), "INTEGER".into(), "REAL".into(), "BOOLEAN".into()],
        supports_auto_increment: false,
        supports_column_comment: false,
        supports_change_column: false,
        supports_rename_column: true,
        uses_foreign_key_keyword: false,
        drop_index_syntax: "simple".into(),
        default_new_column_type: "TEXT".into(),
        uses_database_keyword: false,
        drop_schema_cascade: false,
        is_file_based: false,
        default_port: 0,
        url_schemes: vec![],
        url_template: String::new(),
        fk_violation: None,
        editor_dialect: "sql".into(),
        explain_format: "mysql_json".into(),
        system_databases: vec![],
        file_extensions: vec![],
    }
}

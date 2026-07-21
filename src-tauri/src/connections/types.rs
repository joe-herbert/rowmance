/// Shared data types returned by all database engine implementations.
use serde::{Deserialize, Serialize};

// ── Dialect / SQL generation helpers ─────────────────────────────────────────

/// FK violation error parsing hints — supplied per engine so `TableBrowser` can
/// offer "navigate to referencing table" without knowing which engine is behind
/// the connection.
#[derive(Debug, Serialize, Clone)]
pub struct FkViolationHint {
    /// Literal substring (case-insensitive) that identifies this engine's FK
    /// violation message. Checked against `message + detail`.
    pub detect: String,
    /// Regex string (no flags) with capture group 1 = referencing table name.
    /// Applied case-insensitively to `message + '\n' + detail`.
    #[serde(rename = "tablePattern")]
    pub table_pattern: String,
    /// Regex string with groups 1 = FK column (in referencing table), 2 = referenced
    /// column (in the current table). If matched the filter value is read from the
    /// deleted row, not from the error text.  `null` when not present in the error.
    #[serde(rename = "columnPairPattern")]
    pub column_pair_pattern: Option<String>,
    /// Regex string with groups 1 = referenced column name, 2 = value string
    /// embedded directly in the error (e.g. PostgreSQL DETAIL line).
    /// `null` when the value must be read from the deleted row instead.
    #[serde(rename = "columnValuePattern")]
    pub column_value_pattern: Option<String>,
}

/// Dialect-specific metadata derived from `db_type`, sent to the frontend with
/// every `ConnectionProfile` so the UI never needs to branch on engine name strings.
/// Adding a new engine = populate this struct; no frontend files change.
#[derive(Debug, Serialize, Clone)]
pub struct DialectInfo {
    // ── Identifier quoting ────────────────────────────────────────────────────
    /// Opening identifier quote character, e.g. `` ` ``, `"`, or `[`.
    #[serde(rename = "identifierOpen")]
    pub identifier_open: String,
    /// Closing identifier quote character, e.g. `` ` ``, `"`, or `]`.
    #[serde(rename = "identifierClose")]
    pub identifier_close: String,
    /// Escape sequence for the closing character inside an identifier.
    #[serde(rename = "identifierEscape")]
    pub identifier_escape: String,

    // ── Schema / namespace ────────────────────────────────────────────────────
    /// False for SQLite (no `schema.table` prefix); true for all other engines.
    #[serde(rename = "usesSchema")]
    pub uses_schema: bool,
    /// Human-readable label for the database/schema namespace: "Database" or "Schema".
    #[serde(rename = "dbLabel")]
    pub db_label: String,
    /// True for SQL Server: the connection tree has an instance-database level above
    /// schemas. list_databases() returns instance-level databases; callers must then
    /// call list_schemas() to enumerate schemas within each database.
    #[serde(rename = "hasInstanceDatabases")]
    pub has_instance_databases: bool,
    /// False for PostgreSQL and SQL Server: the connection form allows leaving the
    /// database/schema field blank, connecting at the server/instance level so the
    /// user can browse and pick a database afterwards. True for engines that have no
    /// server-level connection concept (MySQL/MariaDB, Oracle) or are file-based (SQLite).
    #[serde(rename = "requiresDatabase")]
    pub requires_database: bool,

    // ── Query syntax ──────────────────────────────────────────────────────────
    /// True for SQL Server (`SELECT TOP n`); false for engines that use `LIMIT n`.
    #[serde(rename = "selectTop")]
    pub select_top: bool,
    /// True for PostgreSQL (`TRUE`/`FALSE`); false for engines that use `1`/`0`.
    #[serde(rename = "booleanLiterals")]
    pub boolean_literals: bool,
    /// True for PostgreSQL (case-insensitive `ILIKE`); false for engines using `LIKE`.
    #[serde(rename = "usesIlike")]
    pub uses_ilike: bool,
    /// SQL cast expression to convert a column to a searchable text type.
    /// e.g. `"CAST({col} AS TEXT)"`, `"CAST({col} AS NVARCHAR(MAX))"`.
    /// The placeholder `{col}` is replaced by the caller with the quoted column name.
    #[serde(rename = "castToText")]
    pub cast_to_text: String,

    // ── User management capabilities ──────────────────────────────────────────
    /// True when this engine supports listing/creating/dropping users.
    #[serde(rename = "supportsUserManagement")]
    pub supports_user_management: bool,
    /// True when users are identified by `username@host` (MySQL/MariaDB).
    #[serde(rename = "hostBasedUsers")]
    pub host_based_users: bool,
    /// True when the engine supports role-based grants (PostgreSQL).
    #[serde(rename = "supportsRoles")]
    pub supports_roles: bool,

    // ── Editor behaviour ──────────────────────────────────────────────────────
    /// True when the editor should scan for `@varname` SQL variables (MySQL/MariaDB).
    #[serde(rename = "detectsSqlVariables")]
    pub detects_sql_variables: bool,
    /// True when the engine supports per-connection transaction context mismatch
    /// warnings (MySQL/MariaDB only — USE db inside a transaction).
    #[serde(rename = "warnsTxDatabaseMismatch")]
    pub warns_tx_database_mismatch: bool,

    // ── Human-readable display name ───────────────────────────────────────────
    /// Display name shown in the status bar, e.g. "PostgreSQL", "MySQL", "SQLite".
    #[serde(rename = "displayName")]
    pub display_name: String,

    // ── Create-table UI ───────────────────────────────────────────────────────
    /// Default column type shown in the create-table form primary-key column.
    #[serde(rename = "defaultColumnType")]
    pub default_column_type: String,
    /// Ordered list of column types shown in the create-table type picker.
    #[serde(rename = "commonColumnTypes")]
    pub common_column_types: Vec<String>,

    // ── Schema editor capabilities ────────────────────────────────────────────
    /// True when the engine supports `AUTO_INCREMENT` / `AUTOINCREMENT` syntax.
    #[serde(rename = "supportsAutoIncrement")]
    pub supports_auto_increment: bool,
    /// True when column definitions support a `COMMENT` clause (MySQL/MariaDB).
    #[serde(rename = "supportsColumnComment")]
    pub supports_column_comment: bool,
    /// True when the engine supports `ALTER TABLE … CHANGE COLUMN` (MySQL/MariaDB).
    #[serde(rename = "supportsChangeColumn")]
    pub supports_change_column: bool,
    /// True when column renames via `ALTER TABLE … RENAME COLUMN` are supported.
    #[serde(rename = "supportsRenameColumn")]
    pub supports_rename_column: bool,
    /// True when `ALTER TABLE … ADD/DROP FOREIGN KEY` is supported (MySQL/MariaDB
    /// style). False means the engine uses `ADD/DROP CONSTRAINT`.
    #[serde(rename = "usesForeignKeyKeyword")]
    pub uses_foreign_key_keyword: bool,
    /// Syntax for dropping an index. One of: `"on_table"` (MySQL: DROP INDEX x ON t),
    /// `"schema_qualified"` (Postgres: DROP INDEX schema.x), `"on_table_no_schema"` (SQL Server),
    /// or `"simple"` (SQLite: DROP INDEX x).
    #[serde(rename = "dropIndexSyntax")]
    pub drop_index_syntax: String,
    /// Default data type for a new non-PK column in the column form.
    #[serde(rename = "defaultNewColumnType")]
    pub default_new_column_type: String,

    // ── Database/schema DDL ───────────────────────────────────────────────────
    /// True when this engine uses `CREATE DATABASE` / `DROP DATABASE` (MySQL/MariaDB).
    /// False when it uses `CREATE SCHEMA` / `DROP SCHEMA` (Postgres, SQL Server).
    /// SQLite has neither (no CREATE DATABASE command).
    #[serde(rename = "usesDatabaseKeyword")]
    pub uses_database_keyword: bool,
    /// True when `DROP SCHEMA … CASCADE` is required to drop a non-empty schema (Postgres).
    /// False for engines where CASCADE is implicit or unsupported (SQL Server).
    #[serde(rename = "dropSchemaCascade")]
    pub drop_schema_cascade: bool,

    // ── Connection form ───────────────────────────────────────────────────────
    /// True when the engine connects to a local file rather than a host/port (SQLite).
    /// Drives the ConnectionForm UI: file-path input vs host/port/username fields.
    #[serde(rename = "isFileBased")]
    pub is_file_based: bool,
    /// Default TCP port shown in the ConnectionForm, or 0 for file-based engines.
    #[serde(rename = "defaultPort")]
    pub default_port: u16,
    /// URL scheme strings that identify this engine in a pasted connection URL
    /// (e.g. `["mysql"]` for MySQL, `["postgres", "postgresql"]` for PostgreSQL).
    #[serde(rename = "urlSchemes")]
    pub url_schemes: Vec<String>,
    /// URL template used by `connections_get_db_url`.
    /// Placeholders: `{username}`, `{password}`, `{host}`, `{port}`, `{database}`.
    #[serde(rename = "urlTemplate")]
    pub url_template: String,

    // ── FK violation navigation ───────────────────────────────────────────────
    /// Hints for parsing FK constraint errors to offer "navigate to referencing table".
    /// `null` when this engine's FK errors do not include enough info for navigation.
    #[serde(rename = "fkViolation")]
    pub fk_violation: Option<FkViolationHint>,

    // ── Editor / explain ──────────────────────────────────────────────────────
    /// Syntax highlighter dialect for the SQL editor (e.g. "mysql", "postgresql", "sql").
    #[serde(rename = "editorDialect")]
    pub editor_dialect: String,
    /// How to parse EXPLAIN output: "mysql_json" | "postgres_json" | "sqlite_queryplan" | "sqlserver_xml".
    #[serde(rename = "explainFormat")]
    pub explain_format: String,

    // ── System-object filtering ───────────────────────────────────────────────
    /// Database names that are considered system/internal for this engine (e.g.
    /// "information_schema" for MySQL, "postgres" for PostgreSQL). The frontend
    /// merges these with the user's customisable hidden-database list so new
    /// engines are filtered correctly without any manual frontend changes.
    #[serde(rename = "systemDatabases")]
    pub system_databases: Vec<String>,

    // ── File-based engine support ─────────────────────────────────────────────
    /// File extensions recognised by this engine when a file is dragged onto the
    /// app or opened via File > Open (e.g. ["sqlite", "db", "sqlite3"] for SQLite).
    /// Empty for network-based engines.
    #[serde(rename = "fileExtensions")]
    pub file_extensions: Vec<String>,
}

impl DialectInfo {
    pub fn for_db_type(db_type: &str) -> Self {
        crate::connections::engines::dialect_for_db_type(db_type)
    }

    pub fn all_known() -> Vec<(String, Self)> {
        crate::connections::engines::all_known_dialects()
    }
}

#[derive(Debug, Serialize)]
pub struct TableInfo {
    pub name: String,
    #[serde(rename = "tableType")]
    pub table_type: String,
    #[serde(rename = "rowCount")]
    pub row_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
    #[serde(rename = "isAutoIncrement")]
    pub is_auto_increment: bool,
    #[serde(rename = "isForeignKey")]
    pub is_foreign_key: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    #[serde(rename = "indexType")]
    pub index_type: String,
}

#[derive(Debug, Serialize)]
pub struct ForeignKeyInfo {
    #[serde(rename = "constraintName")]
    pub constraint_name: String,
    pub columns: Vec<String>,
    #[serde(rename = "referencedTable")]
    pub referenced_table: String,
    #[serde(rename = "referencedColumns")]
    pub referenced_columns: Vec<String>,
    #[serde(rename = "onDelete")]
    pub on_delete: String,
    #[serde(rename = "onUpdate")]
    pub on_update: String,
}

/// Column metadata included in every query result set.
#[derive(Debug, Serialize, Clone)]
pub struct ColumnMeta {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
    #[serde(rename = "isForeignKey")]
    pub is_foreign_key: bool,
}

/// Raw result returned by an engine's `execute()` call, before IPC wrapping.
pub struct EngineQueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub affected_rows: Option<u64>,
}

/// A row change (update) — primary key values + the new cell values.
#[derive(Deserialize, Debug, Clone)]
pub struct RowChange {
    #[serde(rename = "primaryKeys")]
    pub primary_keys: std::collections::HashMap<String, serde_json::Value>,
    pub changes: std::collections::HashMap<String, serde_json::Value>,
}

/// A row to delete, identified by its primary key column values.
#[derive(Deserialize, Debug, Clone)]
pub struct RowDelete {
    #[serde(rename = "primaryKeys")]
    pub primary_keys: std::collections::HashMap<String, serde_json::Value>,
}

/// Result returned by `DatabaseEngine::explain()`.
#[derive(Debug, Serialize)]
pub struct ExplainResult {
    #[serde(rename = "rawJson")]
    pub raw_json: String,
    pub dialect: String,
}

// ── ERD types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ErdColumn {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
}

#[derive(Debug, Serialize)]
pub struct ErdTable {
    pub name: String,
    pub columns: Vec<ErdColumn>,
}

#[derive(Debug, Serialize)]
pub struct ErdRelation {
    #[serde(rename = "fromTable")]
    pub from_table: String,
    #[serde(rename = "fromColumns")]
    pub from_columns: Vec<String>,
    #[serde(rename = "toTable")]
    pub to_table: String,
    #[serde(rename = "toColumns")]
    pub to_columns: Vec<String>,
    #[serde(rename = "constraintName")]
    pub constraint_name: String,
}

#[derive(Debug, Serialize)]
pub struct ErdGraph {
    pub nodes: Vec<ErdTable>,
    pub edges: Vec<ErdRelation>,
}

// ── User management types ─────────────────────────────────────────────────────

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

// ── Server admin types ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum CapabilityStatus {
    Supported,
    NotSupported,
    InsufficientPrivileges,
    #[serde(rename_all = "camelCase")]
    ExtensionRequired {
        extension: String,
    },
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerAdminCapabilityFlags {
    #[serde(rename = "processList")]
    pub process_list: CapabilityStatus,
    #[serde(rename = "killSession")]
    pub kill_session: CapabilityStatus,
    #[serde(rename = "cancelSession")]
    pub cancel_session: CapabilityStatus,
    #[serde(rename = "serverStatus")]
    pub server_status: CapabilityStatus,
    pub variables: CapabilityStatus,
    #[serde(rename = "setVariable")]
    pub set_variable: CapabilityStatus,
    #[serde(rename = "scheduledJobs")]
    pub scheduled_jobs: CapabilityStatus,
    pub locks: CapabilityStatus,
    #[serde(rename = "innodbStatus")]
    pub innodb_status: CapabilityStatus,
    #[serde(rename = "vacuumStatus")]
    pub vacuum_status: CapabilityStatus,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProcessInfo {
    pub id: String,
    pub user: Option<String>,
    pub host: Option<String>,
    pub database: Option<String>,
    pub command: Option<String>,
    #[serde(rename = "timeSeconds")]
    pub time_seconds: Option<u64>,
    pub state: Option<String>,
    pub info: Option<String>,
    #[serde(rename = "canKill")]
    pub can_kill: bool,
    #[serde(rename = "canCancel")]
    pub can_cancel: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerStatus {
    pub version: String,
    #[serde(rename = "uptimeSeconds")]
    pub uptime_seconds: u64,
    #[serde(rename = "connectionsCurrent")]
    pub connections_current: u64,
    #[serde(rename = "connectionsMax")]
    pub connections_max: Option<u64>,
    #[serde(rename = "queriesPerSecond")]
    pub queries_per_second: Option<f64>,
    #[serde(rename = "cacheHitRatio")]
    pub cache_hit_ratio: Option<f64>,
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum VarScope {
    Session,
    Global,
    Both,
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerVariable {
    pub name: String,
    pub value: String,
    pub scope: VarScope,
    #[serde(rename = "isDynamic")]
    pub is_dynamic: bool,
    #[serde(rename = "restartRequired")]
    pub restart_required: bool,
    pub description: Option<String>,
    #[serde(rename = "dataType")]
    pub data_type: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LockInfo {
    #[serde(rename = "lockId")]
    pub lock_id: String,
    #[serde(rename = "blockerSessionId")]
    pub blocker_session_id: Option<String>,
    #[serde(rename = "waitingSessionId")]
    pub waiting_session_id: Option<String>,
    #[serde(rename = "lockType")]
    pub lock_type: String,
    #[serde(rename = "lockMode")]
    pub lock_mode: String,
    #[serde(rename = "objectName")]
    pub object_name: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
    #[serde(rename = "lastRun")]
    pub last_run: Option<String>,
    #[serde(rename = "nextRun")]
    pub next_run: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VacuumInfo {
    pub table: String,
    #[serde(rename = "lastVacuum")]
    pub last_vacuum: Option<String>,
    #[serde(rename = "lastAutoVacuum")]
    pub last_auto_vacuum: Option<String>,
    #[serde(rename = "deadTuples")]
    pub dead_tuples: i64,
    #[serde(rename = "liveTuples")]
    pub live_tuples: i64,
    #[serde(rename = "bloatEstimateBytes")]
    pub bloat_estimate_bytes: Option<i64>,
}

#[cfg(test)]
mod server_admin_type_tests {
    use super::*;

    // ── CapabilityStatus serialization ────────────────────────────────────────

    #[test]
    fn capability_status_supported_serializes() {
        let json = serde_json::to_string(&CapabilityStatus::Supported).unwrap();
        assert_eq!(json, r#"{"status":"supported"}"#);
    }

    #[test]
    fn capability_status_not_supported_serializes() {
        let json = serde_json::to_string(&CapabilityStatus::NotSupported).unwrap();
        assert_eq!(json, r#"{"status":"notSupported"}"#);
    }

    #[test]
    fn capability_status_insufficient_privileges_serializes() {
        let json = serde_json::to_string(&CapabilityStatus::InsufficientPrivileges).unwrap();
        assert_eq!(json, r#"{"status":"insufficientPrivileges"}"#);
    }

    #[test]
    fn capability_status_extension_required_serializes() {
        let status = CapabilityStatus::ExtensionRequired {
            extension: "pg_cron".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(
            json,
            r#"{"status":"extensionRequired","extension":"pg_cron"}"#
        );
    }

    #[test]
    fn capability_status_eq() {
        assert_eq!(CapabilityStatus::Supported, CapabilityStatus::Supported);
        assert_ne!(CapabilityStatus::Supported, CapabilityStatus::NotSupported);
        assert_eq!(
            CapabilityStatus::ExtensionRequired {
                extension: "x".to_string()
            },
            CapabilityStatus::ExtensionRequired {
                extension: "x".to_string()
            },
        );
        assert_ne!(
            CapabilityStatus::ExtensionRequired {
                extension: "a".to_string()
            },
            CapabilityStatus::ExtensionRequired {
                extension: "b".to_string()
            },
        );
    }

    // ── VarScope serialization / deserialization ──────────────────────────────

    #[test]
    fn var_scope_session_serializes() {
        let json = serde_json::to_string(&VarScope::Session).unwrap();
        assert_eq!(json, r#""session""#);
    }

    #[test]
    fn var_scope_global_serializes() {
        let json = serde_json::to_string(&VarScope::Global).unwrap();
        assert_eq!(json, r#""global""#);
    }

    #[test]
    fn var_scope_both_serializes() {
        let json = serde_json::to_string(&VarScope::Both).unwrap();
        assert_eq!(json, r#""both""#);
    }

    #[test]
    fn var_scope_deserializes_from_camel_case() {
        let session: VarScope = serde_json::from_str(r#""session""#).unwrap();
        assert_eq!(session, VarScope::Session);

        let global: VarScope = serde_json::from_str(r#""global""#).unwrap();
        assert_eq!(global, VarScope::Global);

        let both: VarScope = serde_json::from_str(r#""both""#).unwrap();
        assert_eq!(both, VarScope::Both);
    }

    // ── ServerAdminCapabilityFlags field name serialization ───────────────────

    #[test]
    fn capability_flags_fields_use_camel_case_keys() {
        let flags = ServerAdminCapabilityFlags {
            process_list: CapabilityStatus::Supported,
            kill_session: CapabilityStatus::NotSupported,
            cancel_session: CapabilityStatus::NotSupported,
            server_status: CapabilityStatus::Supported,
            variables: CapabilityStatus::Supported,
            set_variable: CapabilityStatus::Supported,
            scheduled_jobs: CapabilityStatus::InsufficientPrivileges,
            locks: CapabilityStatus::Supported,
            innodb_status: CapabilityStatus::NotSupported,
            vacuum_status: CapabilityStatus::NotSupported,
        };
        let json = serde_json::to_value(&flags).unwrap();
        assert!(json.get("processList").is_some(), "processList key missing");
        assert!(json.get("killSession").is_some(), "killSession key missing");
        assert!(
            json.get("cancelSession").is_some(),
            "cancelSession key missing"
        );
        assert!(
            json.get("serverStatus").is_some(),
            "serverStatus key missing"
        );
        assert!(json.get("setVariable").is_some(), "setVariable key missing");
        assert!(
            json.get("scheduledJobs").is_some(),
            "scheduledJobs key missing"
        );
        assert!(
            json.get("innodbStatus").is_some(),
            "innodbStatus key missing"
        );
        assert!(
            json.get("vacuumStatus").is_some(),
            "vacuumStatus key missing"
        );
        // snake_case keys must not appear
        assert!(json.get("process_list").is_none());
        assert!(json.get("kill_session").is_none());
    }
}

/// Flat column row used by `schema_list_all_columns` for bulk column fetching.
#[derive(Debug, Serialize)]
pub struct BulkColumnRow {
    #[serde(rename = "tableName")]
    pub table_name: String,
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(rename = "isPrimaryKey")]
    pub is_primary_key: bool,
    #[serde(rename = "isAutoIncrement")]
    pub is_auto_increment: bool,
    #[serde(rename = "isForeignKey")]
    pub is_foreign_key: bool,
    pub comment: Option<String>,
}

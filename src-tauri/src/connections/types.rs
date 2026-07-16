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
#[derive(Deserialize, Debug)]
pub struct RowChange {
    #[serde(rename = "primaryKeys")]
    pub primary_keys: std::collections::HashMap<String, serde_json::Value>,
    pub changes: std::collections::HashMap<String, serde_json::Value>,
}

/// A row to delete, identified by its primary key column values.
#[derive(Deserialize, Debug)]
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

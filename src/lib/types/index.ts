/** Shared TypeScript types used across the frontend and Tauri invoke wrappers. */

// ── Database connections ────────────────────────────────────────────────────

export type DbType = 'mysql' | 'mariadb' | 'postgres' | 'sqlite';

/** A connection profile as stored in the local SQLite database. */
export interface ConnectionProfile {
  id: string;
  groupId: string | null;
  name: string;
  dbType: DbType;
  host: string;
  port: number;
  database: string;
  username: string;
  /** Hex colour string, e.g. '#4f46e5'. */
  color: string | null;
  readOnly: boolean;
  sshEnabled: boolean;
  sshHost: string | null;
  sshPort: number | null;
  sshUser: string | null;
  sshAuthType: 'password' | 'key' | null;
  sshKeyPath: string | null;
  sslEnabled: boolean;
  sslCaPath: string | null;
  sslCertPath: string | null;
  sslKeyPath: string | null;
  poolMin: number;
  poolMax: number;
  createdAt: string;
  updatedAt: string;
}

/** Input type for creating or updating a connection profile.
 * Passwords are stored in the OS keychain via keychainStore(), never here. */
export interface ConnectionProfileInput {
  groupId?: string | null;
  name: string;
  dbType: DbType;
  host: string;
  port: number;
  database: string;
  username: string;
  color?: string | null;
  readOnly?: boolean;
  sshEnabled?: boolean;
  sshHost?: string | null;
  sshPort?: number | null;
  sshUser?: string | null;
  sshAuthType?: 'password' | 'key' | null;
  sshKeyPath?: string | null;
  sslEnabled?: boolean;
  sslCaPath?: string | null;
  sslCertPath?: string | null;
  sslKeyPath?: string | null;
  poolMin?: number;
  poolMax?: number;
}

export interface ConnectionGroup {
  id: string;
  name: string;
  parentId: string | null;
  position: number;
}

export interface ConnectionGroupInput {
  name: string;
  parentId?: string | null;
  position?: number;
}

export interface ConnectionTestResult {
  success: boolean;
  message: string;
  latencyMs: number | null;
}

// ── Query results ────────────────────────────────────────────────────────────

export interface ColumnMeta {
  name: string;
  /** SQL type string as reported by the database, e.g. 'VARCHAR(255)', 'INT'. */
  dataType: string;
  nullable: boolean;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
  defaultValue?: string | null;
  isAutoIncrement?: boolean;
  isUnique?: boolean;
}

/** A single query result set returned from the backend. */
export interface QueryResult {
  queryId: string;
  columns: ColumnMeta[];
  /** Rows as arrays of JSON values; null represents SQL NULL. */
  rows: (string | number | boolean | null)[][];
  /** Total row count before pagination (from COUNT(*) sub-query). */
  totalRows: number | null;
  durationMs: number;
  affectedRows: number | null;
  /** Error message, present when the query failed. */
  error: string | null;
}

// ── Schema ───────────────────────────────────────────────────────────────────

export interface TableInfo {
  name: string;
  tableType: 'table' | 'view';
  rowCount: number | null;
}

export interface ColumnInfo {
  name: string;
  dataType: string;
  nullable: boolean;
  defaultValue: string | null;
  isPrimaryKey: boolean;
  isAutoIncrement: boolean;
  isForeignKey: boolean;
  comment: string | null;
}

export interface IndexInfo {
  name: string;
  columns: string[];
  unique: boolean;
  indexType: string;
}

export interface ForeignKeyInfo {
  constraintName: string;
  columns: string[];
  referencedTable: string;
  referencedColumns: string[];
  onDelete: string;
  onUpdate: string;
}

/** Full schema for a single database. */
export interface DatabaseSchema {
  database: string;
  tables: TableInfo[];
}

// ── Query history ─────────────────────────────────────────────────────────────

export interface QueryHistoryEntry {
  id: string;
  connectionId: string;
  sql: string;
  executedAt: string;
  durationMs: number | null;
  rowCount: number | null;
  error: string | null;
  status: 'success' | 'error' | 'cancelled';
}

// ── Saved queries ─────────────────────────────────────────────────────────────

export interface SavedQueryFolder {
  id: string;
  name: string;
  parentId: string | null;
  position: number;
}

export interface SavedQuery {
  id: string;
  connectionId: string | null;
  folderId: string | null;
  name: string;
  sql: string;
  createdAt: string;
  updatedAt: string;
}

// ── ERD ──────────────────────────────────────────────────────────────────────

export interface ErdColumn { name: string; dataType: string; isPrimaryKey: boolean; }
export interface ErdTable { name: string; columns: ErdColumn[]; }
export interface ErdRelation { fromTable: string; fromColumns: string[]; toTable: string; toColumns: string[]; constraintName: string; }
export interface ErdGraph { nodes: ErdTable[]; edges: ErdRelation[]; }

// ── EXPLAIN ──────────────────────────────────────────────────────────────────

export interface ExplainResult { rawJson: string; dialect: string; }

// ── Themes ───────────────────────────────────────────────────────────────────

export interface ThemeMeta { name: string; extends: string; }
export interface ThemeData { name: string; extends: string; variables: Record<string, string>; }

// ── Updater ──────────────────────────────────────────────────────────────────

export interface UpdateCheckResult { available: boolean; version: string | null; notes: string | null; }

// ── Panels ───────────────────────────────────────────────────────────────────

export type PanelKind =
  | { kind: 'query_editor'; connectionId: string; initialSql?: string; editorId?: string }
  | { kind: 'table_browser'; connectionId: string; database: string; table: string; initialFilter?: string }
  | { kind: 'table_structure'; connectionId: string; database: string; table: string }
  | { kind: 'ddl_viewer'; connectionId: string; database: string; objectName: string; objectType: 'table' | 'view' }
  | { kind: 'erd'; connectionId: string; database: string }
  | { kind: 'explain'; connectionId: string; sql: string; dialect: string }
  | { kind: 'settings' }
  | { kind: 'empty' };

export interface PanelState {
  id: string;
  content: PanelKind;
}

export type SplitMode = 'none' | 'horizontal' | 'vertical' | 'quad';

// ── Settings ─────────────────────────────────────────────────────────────────

export interface AppSettings {
  theme: string;
  pageSize: number;
  autoSaveEdits: boolean;
  showGeneratedSql: boolean;
  editorTabSize: number;
  editorLineWrap: boolean;
  fontFamily: string;
  fontSize: number;
  shortcutPreset: 'vscode' | 'vim' | 'jetbrains';
  autoUpdateCheck: boolean;
  transactionMode: boolean;
  historyMaxEntries: number;
  sidebarFloating: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'system',
  pageSize: 50,
  autoSaveEdits: false,
  showGeneratedSql: false,
  editorTabSize: 2,
  editorLineWrap: false,
  fontFamily: 'system-ui',
  fontSize: 13,
  shortcutPreset: 'vscode',
  autoUpdateCheck: true,
  transactionMode: false,
  historyMaxEntries: 500,
  sidebarFloating: false,
};

// ── Errors ───────────────────────────────────────────────────────────────────

/** Structured error returned by the Rust backend via Tauri IPC. */
export interface AppError {
  code: string;
  message: string;
  detail: string | null;
}

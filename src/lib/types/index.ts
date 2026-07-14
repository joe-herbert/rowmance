/** Shared TypeScript types used across the frontend and Tauri invoke wrappers. */
import { BUILTIN_DATABASES, BUILTIN_TABLE_PATTERNS } from '$lib/utils/system-items';

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
  poolMax: number;
  pingInterval: number | null;
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
  poolMax?: number;
  pingInterval?: number | null;
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
  durationUs: number;
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

export interface BulkColumnInfo extends ColumnInfo {
  tableName: string;
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
  durationUs: number | null;
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
  database: string | null;
  name: string;
  sql: string;
  description: string | null;
  annotations: string | null;
  position: number;
  createdAt: string;
  updatedAt: string;
}

// ── File-based saved queries ──────────────────────────────────────────────────

/** Connection resolution status for a file-based query. */
export type ConnectionStatus = 'resolved' | 'fingerprint_matched' | 'unresolved' | 'none';

/** A saved query loaded from a .sql file on disk. */
export interface FileQuery {
  /** Relative path from the queries directory, e.g. "reports/monthly.sql". */
  id: string;
  folderId: string | null;
  name: string;
  sql: string;
  description: string | null;
  annotations: string | null;
  /** Resolved local connection ID (null when unresolved or none). */
  connectionId: string | null;
  /** How the connection was resolved. */
  connectionStatus: ConnectionStatus;
  /** Raw connection_id stored in the file header (may belong to another user). */
  fileConnectionId: string | null;
  /** Connection fingerprint stored in the file header. */
  fileFingerprint: string | null;
  database: string | null;
  position: number;
  createdAt: string;
  updatedAt: string;
}

export interface FileQueryFolder {
  /** Relative path from the queries directory, e.g. "reports". */
  id: string;
  parentId: string | null;
  name: string;
  position: number;
}

export interface FileQueryListResult {
  folders: FileQueryFolder[];
  queries: FileQuery[];
}

// ── ERD ──────────────────────────────────────────────────────────────────────

export interface ErdColumn {
  name: string;
  dataType: string;
  isPrimaryKey: boolean;
}
export interface ErdTable {
  name: string;
  columns: ErdColumn[];
}
export interface ErdRelation {
  fromTable: string;
  fromColumns: string[];
  toTable: string;
  toColumns: string[];
  constraintName: string;
}
export interface ErdGraph {
  nodes: ErdTable[];
  edges: ErdRelation[];
}

// ── Virtual Relations ─────────────────────────────────────────────────────────

export interface ColumnRef {
  connectionId: string;
  database: string;
  table: string;
  column: string;
}

export interface VirtualRelation {
  id: string;
  label?: string;
  from: ColumnRef;
  to: ColumnRef;
}

export interface PolymorphicMapping {
  id: string;
  typeValue: string;
  to: ColumnRef;
}

export interface PolymorphicVirtualRelation {
  id: string;
  label?: string;
  connectionId: string;
  database: string;
  table: string;
  typeColumn: string;
  valueColumn: string;
  mappings: PolymorphicMapping[];
}

// ── EXPLAIN ──────────────────────────────────────────────────────────────────

export interface ExplainResult {
  rawJson: string;
  dialect: string;
}

// ── Themes ───────────────────────────────────────────────────────────────────

export interface ThemeMeta {
  name: string;
  extends: string;
}
export interface ThemeData {
  name: string;
  extends: string;
  variables: Record<string, string>;
}

// ── Updater ──────────────────────────────────────────────────────────────────

export interface UpdateCheckResult {
  available: boolean;
  version: string | null;
  notes: string | null;
}

// ── Users ────────────────────────────────────────────────────────────────────

export interface DbUser {
  username: string;
  host: string | null;
  isSuperuser: boolean;
  canCreateDb: boolean;
  canCreateRole: boolean;
  isLocked: boolean;
}

// ── Panels ───────────────────────────────────────────────────────────────────

export type PanelKind =
  | {
      kind: 'query_editor';
      connectionId: string;
      database?: string;
      initialSql?: string;
      initialDescription?: string;
      initialAnnotations?: string;
      editorId?: string;
      savedQueryId?: string;
      savedQueryName?: string;
    }
  | {
      kind: 'table_browser';
      connectionId: string;
      database: string;
      table: string;
      initialFilter?: string;
    }
  | { kind: 'table_structure'; connectionId: string; database: string; table: string }
  | {
      kind: 'ddl_viewer';
      connectionId: string;
      database: string;
      objectName: string;
      objectType: 'table' | 'view';
    }
  | { kind: 'erd'; connectionId: string; database: string }
  | { kind: 'explain'; connectionId: string; sql: string; dialect: string }
  | { kind: 'settings' }
  | { kind: 'user_manager'; connectionId: string }
  | { kind: 'speed_analysis' }
  | { kind: 'release_notes'; version: string; notes: string }
  | { kind: 'connections' }
  | { kind: 'dashboard'; dashboardId: string }
  | { kind: 'empty' };

export interface PanelState {
  id: string;
  content: PanelKind;
}

export type SplitNode =
  | { type: 'leaf'; splitId: string }
  | {
      type: 'container';
      containerId: string;
      direction: 'horizontal' | 'vertical';
      children: SplitChild[];
    };

export interface SplitChild {
  node: SplitNode;
  /** Percentage 0–100; all siblings in the same container sum to 100. */
  size: number;
}

// ── Settings ─────────────────────────────────────────────────────────────────

export type AiProvider = 'none' | 'claude' | 'openai' | 'gemini' | 'ollama' | 'custom';
export type AiContextLevel = 'none' | 'structure' | 'structure_and_data';

export type SoftDeleteConditionType = 'not-null' | 'is-null' | 'true' | 'false' | 'equals';

export interface SoftDeleteCondition {
  column: string;
  type: SoftDeleteConditionType;
  /** Only used when type is 'equals'. */
  value?: string;
}

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
  historyMaxEntries: number;
  sidebarFloating: boolean;
  leftSidebarVisible: boolean;
  rightSidebarVisible: boolean;
  rightSidebarPanel: string;
  clickOutsideEdit: 'discard' | 'confirm';
  nowTimeSource: 'user' | 'database';
  booleanDisplay: 'tick-cross' | 'true-false' | '1-0' | 'as-saved';
  systemDatabases: string[];
  systemTablePatterns: string[];
  showSystemItems: boolean;
  formatKeywordCase: 'upper' | 'lower' | 'preserve';
  formatIndentStyle: 'standard' | 'tabularLeft' | 'tabularRight';
  formatLinesBetweenQueries: number;
  formatCompact: boolean;
  /** Controls overall formatting density. Takes priority over formatCompact. */
  formatStyle: 'expanded' | 'comfortable' | 'compact';
  openItemsLocation: 'sidebar' | 'top';
  maxHorizontalSplits: number;
  maxVerticalSplits: number;
  cellMaxLines: number;
  newlineReplacement: string;
  newRowPosition: 'top' | 'bottom';
  confirmBeforeDelete: boolean;
  /** Absolute path to the saved queries directory. Empty string = use default. */
  savedQueriesDirectory: string;
  localSearchHighlight: boolean;
  /** Automatically save a saved query when it is run and has unsaved changes. */
  saveOnRun: boolean;
  aiProvider: AiProvider;
  aiModel: string;
  aiApiKey: string;
  aiBaseUrl: string;
  aiContextLevel: AiContextLevel;
  aiDataSampleRows: number;
  /** Highlight rows that appear to be soft-deleted based on column values. */
  softDeleteHighlight: boolean;
  /** Strike through text in soft-deleted rows. */
  softDeleteStrikethrough: boolean;
  /** Column conditions used to detect soft-deleted rows. */
  softDeleteConditions: SoftDeleteCondition[];
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
  historyMaxEntries: 500,
  sidebarFloating: false,
  leftSidebarVisible: true,
  rightSidebarVisible: false,
  rightSidebarPanel: 'history',
  clickOutsideEdit: 'discard',
  nowTimeSource: 'user',
  booleanDisplay: 'tick-cross',
  systemDatabases: [...BUILTIN_DATABASES],
  systemTablePatterns: [...BUILTIN_TABLE_PATTERNS],
  showSystemItems: true,
  formatKeywordCase: 'upper',
  formatIndentStyle: 'standard',
  formatLinesBetweenQueries: 1,
  formatCompact: false,
  formatStyle: 'expanded',
  openItemsLocation: 'sidebar',
  maxHorizontalSplits: 2,
  maxVerticalSplits: 2,
  cellMaxLines: 1,
  newlineReplacement: '↵',
  newRowPosition: 'bottom',
  confirmBeforeDelete: true,
  savedQueriesDirectory: '',
  localSearchHighlight: true,
  saveOnRun: false,
  aiProvider: 'none',
  aiModel: '',
  aiApiKey: '',
  aiBaseUrl: '',
  aiContextLevel: 'none',
  aiDataSampleRows: 3,
  softDeleteHighlight: true,
  softDeleteStrikethrough: true,
  softDeleteConditions: [
    { column: 'deleted_at', type: 'not-null' },
    { column: 'removed_at', type: 'not-null' },
    { column: 'discarded_at', type: 'not-null' },
    { column: 'archived_at', type: 'not-null' },
    { column: 'disabled_at', type: 'not-null' },
    { column: 'deactivated_at', type: 'not-null' },
    { column: 'deleted_on', type: 'not-null' },
    { column: 'expired_at', type: 'not-null' },
    { column: 'retired_at', type: 'not-null' },
    { column: 'purged_at', type: 'not-null' },
    { column: 'deletion_date', type: 'not-null' },
    { column: 'deletion_timestamp', type: 'not-null' },
    { column: 'soft_deleted_at', type: 'not-null' },
    { column: 'trashed_at', type: 'not-null' },
    { column: 'deleted', type: 'true' },
    { column: 'is_deleted', type: 'true' },
    { column: 'active', type: 'false' },
    { column: 'is_active', type: 'false' },
    { column: 'status', type: 'equals', value: 'deleted' },
  ],
};

// ── Dashboards ───────────────────────────────────────────────────────────────

export type WidgetDisplayType = 'count' | 'table' | 'bar_chart' | 'line_chart' | 'countdown';

export type SingleValueFormat =
  | 'auto'
  | 'number'
  | 'number_compact'
  | 'currency'
  | 'percent'
  | 'date'
  | 'datetime'
  | 'boolean'
  | 'text';

export interface DashboardWidget {
  id: string;
  title: string;
  /** Width in 12-column grid (1–12) */
  w: number;
  /** Height in row units (each 80px) */
  h: number;
  /** Column start, 1-indexed */
  x: number;
  /** Row start, 1-indexed */
  y: number;
  connectionId: string;
  database: string | null;
  sql: string;
  displayType: WidgetDisplayType;
  /** Format for single-value (count) display type */
  singleValueFormat?: SingleValueFormat;
  /** ISO 4217 currency code, used when singleValueFormat === 'currency' */
  singleValueCurrency?: string;
}

export interface Dashboard {
  id: string;
  name: string;
  /** SVG string for the icon */
  icon: string;
  pinned: boolean;
  pinnedOrder: number | null;
  widgets: DashboardWidget[];
  createdAt: string;
  updatedAt: string;
}

// ── Errors ───────────────────────────────────────────────────────────────────

/** Structured error returned by the Rust backend via Tauri IPC. */
export interface AppError {
  code: string;
  message: string;
  detail: string | null;
}

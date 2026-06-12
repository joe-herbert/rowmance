# Rowmance — Project Plan

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Tech Stack](#2-tech-stack)
3. [Architecture Overview](#3-architecture-overview)
4. [Directory Structure](#4-directory-structure)
5. [Local SQLite Schema](#5-local-sqlite-schema)
6. [Tauri Command Surface](#6-tauri-command-surface)
7. [Rust Backend Design](#7-rust-backend-design)
8. [Frontend Architecture](#8-frontend-architecture)
9. [Feature Breakdown](#9-feature-breakdown)
10. [Layout System](#10-layout-system)
11. [Theme System](#11-theme-system)
12. [Keyboard Shortcut System](#12-keyboard-shortcut-system)
13. [Development Standards](#13-development-standards)
14. [Makefile](#14-makefile)
15. [Phased Implementation Plan](#15-phased-implementation-plan)
16. [Key Design Decisions](#16-key-design-decisions)
17. [Risk Register](#17-risk-register)
18. [Dependency Reference](#18-dependency-reference)

---

## 1. Project Overview

**Rowmance** is a native desktop database client for MySQL, MariaDB, and PostgreSQL, built with Tauri v2 (Rust backend) and Svelte 5 (frontend). It provides a split-panel workspace with a query editor, table browser, schema explorer, ERD viewer, and rich data editing — all in a single, keyboard-navigable interface.

### Goals

- Professional-grade query and data editing experience comparable to TablePlus or DataGrip
- Fast, native-feeling UI with no Electron overhead
- Fully themeable via CSS variables with an in-app editor
- Secure credential storage via OS keychain
- SSH tunnel and SSL/TLS support for remote connections
- Self-updating via GitHub Releases

### Non-Goals (v1)

- NoSQL database support (MongoDB, Redis, etc.)
- Cloud database management (schema migrations as code, drift detection)
- Team collaboration or cloud sync features
- Query builder GUI (focus is on SQL authoring)

---

## 2. Tech Stack

### Runtime and Build

| Layer | Choice | Version / Notes |
|---|---|---|
| Desktop runtime | Tauri | v2 |
| Backend language | Rust | stable |
| Frontend language | TypeScript via Svelte | Svelte 5 (runes API) |
| Package manager | bun | latest stable |
| Bundler | Vite | via `@tauri-apps/cli` |

### Rust Crates

| Crate | Purpose |
|---|---|
| `sqlx` | Async SQL for MySQL, PostgreSQL, and SQLite (`features = ["mysql", "postgres", "sqlite", "runtime-tokio-rustls"]`) |
| `keyring` | OS keychain access (macOS Keychain, Windows Credential Manager, libsecret on Linux) |
| `russh` + `russh-keys` | SSH tunnel establishment and port forwarding |
| `tauri-plugin-updater` | Auto-update via GitHub Releases |
| `tauri-plugin-fs` | Filesystem access (theme files, cert paths) |
| `tauri-plugin-dialog` | Native file open/save dialogs |
| `tauri-plugin-shell` | Shell integration if needed |
| `serde` + `serde_json` | Serialisation for IPC and SQLite JSON columns |
| `tokio` | Async runtime |
| `uuid` | ID generation (`features = ["v4"]`) |
| `anyhow` | Ergonomic error propagation in application code |
| `thiserror` | Typed error enums for domain errors |
| `csv` | CSV parsing for import |
| `dashmap` | Concurrent `HashMap` for active connection pool registry |

### Frontend npm Packages

| Package | Purpose |
|---|---|
| `svelte` (v5) | UI framework, runes API |
| `@tauri-apps/api` + plugins | Tauri IPC, filesystem, dialog, updater |
| `@tanstack/svelte-table` | Headless table logic |
| `bits-ui` | Accessible headless component primitives |
| `codemirror` + `@codemirror/lang-sql` | SQL editor core |
| `@codemirror/autocomplete` | Completion framework |
| `@codemirror/view` + `@codemirror/state` | Editor primitives |
| `@codemirror/theme-one-dark` | Starting point for dark theme |
| `fuse.js` | Fuzzy search (schema tree, command palette) |
| `elkjs` + `web-worker` | ERD automatic graph layout |
| `d3-zoom` + `d3-drag` | ERD pan/zoom interactions |
| `sql-formatter` | Query formatting |

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Svelte 5 Frontend                           │
│                                                                     │
│  Rune-based stores  →  Component tree  →  Tauri invoke() calls     │
│  (panels, connections, settings, shortcuts)                         │
└───────────────────────────────┬─────────────────────────────────────┘
                                │  IPC (JSON over Tauri)
┌───────────────────────────────▼─────────────────────────────────────┐
│                        Tauri v2 Core                                │
│                                                                     │
│  Command handlers (src-tauri/src/commands/)                         │
│  Registered in lib.rs via .invoke_handler()                         │
└──┬──────────────────┬──────────────┬──────────────┬─────────────────┘
   │                  │              │              │
   ▼                  ▼              ▼              ▼
Pool Manager    SQLite (sqlx)   Keychain         SSH Tunnel
(dashmap of     ~/.config/      (keyring)        Manager
 sqlx pools     rowmance/       OS-native        (russh)
 per conn ID)   rowmance.db
   │
   ▼
MySQL / MariaDB / PostgreSQL
(remote databases, via plain TCP or through SSH-forwarded local port)
```

### IPC Contract

All data crossing the Tauri IPC boundary is serialised to JSON. Rust structs that cross the boundary derive `serde::Serialize` and `serde::Deserialize`. Errors are returned as structured JSON objects, never raw strings, so the frontend can display user-friendly messages.

```rust
// Consistent error envelope
#[derive(Serialize)]
pub struct AppError {
    pub code: &'static str,   // machine-readable, e.g. "CONNECTION_REFUSED"
    pub message: String,      // human-readable
    pub detail: Option<String>, // optional stack/context
}
```

The frontend wraps every `invoke()` call in a typed helper (in `src/lib/tauri/`) that knows the expected return shape and maps errors to the `AppError` type.

---

## 4. Directory Structure

```
rowmance/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs                    — Tauri entry point, app bootstrap
│   │   ├── lib.rs                     — Command registration, plugin setup
│   │   ├── error.rs                   — AppError type + From impls
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── connections.rs         — CRUD profiles, test, list active pools
│   │   │   ├── query.rs               — execute, cancel, explain, format sql
│   │   │   ├── schema.rs              — databases/tables/columns/DDL/indexes/FKs
│   │   │   ├── history.rs             — list, clear, search query history
│   │   │   ├── saved_queries.rs       — CRUD folders and queries
│   │   │   ├── settings.rs            — get/set key-value settings
│   │   │   ├── keychain.rs            — store/retrieve/delete credentials
│   │   │   ├── ssh.rs                 — create/destroy SSH tunnels
│   │   │   ├── erd.rs                 — fetch FK graph data for ERD
│   │   │   ├── import.rs              — CSV import, SQL file import
│   │   │   ├── export.rs              — export result set to file/clipboard
│   │   │   ├── themes.rs              — list/read/write theme files
│   │   │   └── updater.rs             — check for updates, install
│   │   ├── db/
│   │   │   ├── mod.rs                 — SQLitePool initialisation, migration runner
│   │   │   ├── models.rs              — SQLite row structs
│   │   │   └── migrations/
│   │   │       ├── 0001_init.sql
│   │   │       └── 0002_*.sql         — future migrations
│   │   └── connections/
│   │       ├── mod.rs
│   │       ├── pool_manager.rs        — DashMap<Uuid, AnyPool>, connect/disconnect
│   │       ├── mysql.rs               — MySQL/MariaDB-specific introspection queries
│   │       ├── postgres.rs            — PostgreSQL-specific introspection queries
│   │       └── ssh_tunnel.rs          — russh-based tunnel lifecycle
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
├── src/
│   ├── app.svelte                     — Root component, layout shell
│   ├── app.html                       — HTML entry point
│   ├── lib/
│   │   ├── components/
│   │   │   ├── layout/
│   │   │   │   ├── Sidebar.svelte         — Left sidebar container + resize logic
│   │   │   │   ├── SidebarTopHalf.svelte  — Open editors/tables list
│   │   │   │   ├── SidebarBottomHalf.svelte — Schema tree host
│   │   │   │   ├── SplitPanel.svelte      — 2x2 panel grid with drag splits
│   │   │   │   ├── Panel.svelte           — Individual panel with type routing
│   │   │   │   └── RightSidebar.svelte    — Toggleable right sidebar
│   │   │   ├── editor/
│   │   │   │   ├── QueryEditor.svelte     — CodeMirror 6 wrapper
│   │   │   │   └── ResultsPanel.svelte    — Results below editor
│   │   │   ├── table/
│   │   │   │   ├── DataTable.svelte       — TanStack Table wrapper
│   │   │   │   ├── CellEditor.svelte      — Inline edit: text/null/bool/date
│   │   │   │   ├── ColumnPicker.svelte    — Show/hide columns popover
│   │   │   │   └── TableBrowser.svelte    — Table view with filter/sort bar
│   │   │   ├── schema/
│   │   │   │   ├── SchemaTree.svelte      — Fuse.js-powered tree
│   │   │   │   └── DdlViewer.svelte       — DDL read-only CodeMirror panel
│   │   │   ├── erd/
│   │   │   │   └── ErdCanvas.svelte       — SVG ERD with elkjs layout
│   │   │   ├── connections/
│   │   │   │   ├── ConnectionForm.svelte  — Create/edit connection dialog
│   │   │   │   └── ConnectionTree.svelte  — Grouped connection list
│   │   │   ├── palette/
│   │   │   │   └── CommandPalette.svelte  — Cmd+K fuzzy palette
│   │   │   └── settings/
│   │   │       ├── Settings.svelte        — Settings page shell + nav
│   │   │       ├── ThemeEditor.svelte     — Visual theme variable editor
│   │   │       └── KeyboardShortcuts.svelte — Shortcut binding table
│   │   ├── stores/
│   │   │   ├── connections.svelte.ts      — Active connections, pool status
│   │   │   ├── panels.svelte.ts           — Panel layout state
│   │   │   ├── settings.svelte.ts         — App settings (synced to backend)
│   │   │   └── shortcuts.svelte.ts        — Effective shortcut map
│   │   ├── tauri/
│   │   │   ├── connections.ts             — Typed invoke wrappers
│   │   │   ├── query.ts
│   │   │   ├── schema.ts
│   │   │   ├── history.ts
│   │   │   ├── saved_queries.ts
│   │   │   ├── settings.ts
│   │   │   ├── keychain.ts
│   │   │   ├── ssh.ts
│   │   │   ├── erd.ts
│   │   │   ├── import.ts
│   │   │   ├── export.ts
│   │   │   ├── themes.ts
│   │   │   └── updater.ts
│   │   └── utils/
│   │       ├── format.ts                  — Date, number, byte formatting
│   │       ├── sql.ts                     — Statement splitter, cursor detection
│   │       └── shortcuts.ts               — Shortcut normalisation, matching
│   └── styles/
│       ├── variables.css                  — Master CSS variable definitions
│       ├── base.css                       — Reset, typography, base rules
│       └── themes/
│           ├── light.css                  — Light theme variable overrides
│           └── dark.css                   — Dark theme variable overrides
├── package.json
├── bun.lockb
├── tsconfig.json
├── vite.config.ts
└── PLAN.md
```

---

## 5. Local SQLite Schema

The local SQLite database lives at `~/.config/rowmance/rowmance.db`. All migrations run at startup via `sqlx::migrate!()` pointed at `src-tauri/src/db/migrations/`.

### Migration 0001 — Initial Schema

```sql
-- Connection organisation
CREATE TABLE connection_groups (
    id          TEXT PRIMARY KEY,   -- UUID v4
    name        TEXT NOT NULL,
    parent_id   TEXT REFERENCES connection_groups(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0
);

-- Connection profiles (no passwords — stored in keychain keyed by id)
CREATE TABLE connection_profiles (
    id              TEXT PRIMARY KEY,
    group_id        TEXT REFERENCES connection_groups(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    db_type         TEXT NOT NULL CHECK (db_type IN ('mysql', 'mariadb', 'postgres')),
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL,
    database        TEXT NOT NULL,
    username        TEXT NOT NULL,
    color           TEXT,               -- hex string, e.g. '#4f46e5'
    read_only       INTEGER NOT NULL DEFAULT 0,
    -- SSH tunnel
    ssh_enabled     INTEGER NOT NULL DEFAULT 0,
    ssh_host        TEXT,
    ssh_port        INTEGER DEFAULT 22,
    ssh_user        TEXT,
    ssh_auth_type   TEXT CHECK (ssh_auth_type IN ('password', 'key', NULL)),
    ssh_key_path    TEXT,
    -- SSL/TLS
    ssl_enabled     INTEGER NOT NULL DEFAULT 0,
    ssl_ca_path     TEXT,
    ssl_cert_path   TEXT,
    ssl_key_path    TEXT,
    -- Connection pool
    pool_min        INTEGER NOT NULL DEFAULT 1,
    pool_max        INTEGER NOT NULL DEFAULT 5,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Per-query execution log
CREATE TABLE query_history (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT NOT NULL REFERENCES connection_profiles(id) ON DELETE CASCADE,
    sql             TEXT NOT NULL,
    executed_at     TEXT NOT NULL DEFAULT (datetime('now')),
    duration_ms     INTEGER,
    row_count       INTEGER,
    error           TEXT,
    status          TEXT NOT NULL CHECK (status IN ('success', 'error', 'cancelled'))
);
CREATE INDEX idx_query_history_connection ON query_history(connection_id, executed_at DESC);

-- Saved query organisation
CREATE TABLE saved_query_folders (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_id   TEXT REFERENCES saved_query_folders(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0
);

-- Saved queries
CREATE TABLE saved_queries (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT REFERENCES connection_profiles(id) ON DELETE SET NULL,
    folder_id       TEXT REFERENCES saved_query_folders(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    sql             TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Flat key-value settings store (values are JSON-encoded)
CREATE TABLE settings (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL  -- JSON
);

-- Per-action keyboard shortcut overrides
CREATE TABLE keyboard_shortcut_overrides (
    action      TEXT PRIMARY KEY,
    shortcut    TEXT NOT NULL,  -- e.g. "Meta+Enter"
    preset      TEXT NOT NULL DEFAULT 'vscode'
);
```

### Default Settings Keys

| Key | Default Value | Description |
|---|---|---|
| `theme` | `"system"` | `"light"`, `"dark"`, or `"system"` |
| `page_size` | `50` | Default results page size |
| `auto_save_edits` | `false` | Auto-apply cell edits immediately |
| `show_generated_sql` | `false` | Show SQL after saving cell edits |
| `editor_tab_size` | `2` | CodeMirror tab width |
| `editor_line_wrap` | `false` | Wrap long lines in editor |
| `editor_autocomplete_trigger` | `"auto"` | `"auto"` or `"manual"` |
| `font_family` | `"system-ui"` | UI font |
| `font_size` | `13` | Base font size in px |
| `shortcut_preset` | `"vscode"` | Active shortcut preset |
| `auto_update_check` | `true` | Check for updates on startup |
| `transaction_mode` | `false` | Enable transaction control by default |
| `history_max_entries` | `500` | How many history items to retain per connection |

---

## 6. Tauri Command Surface

All commands are registered in `lib.rs` via `.invoke_handler(tauri::generate_handler![...])`. Each module in `commands/` provides a set of `#[tauri::command]` async functions.

### 6.1 `connections::*`

| Command | Inputs | Returns |
|---|---|---|
| `connections_list` | — | `Vec<ConnectionProfile>` |
| `connections_create` | `ConnectionProfileInput` | `ConnectionProfile` |
| `connections_update` | `id: Uuid`, `ConnectionProfileInput` | `ConnectionProfile` |
| `connections_delete` | `id: Uuid` | `()` |
| `connections_test` | `id: Uuid` | `ConnectionTestResult` |
| `connections_connect` | `id: Uuid` | `()` |
| `connections_disconnect` | `id: Uuid` | `()` |
| `connections_list_active` | — | `Vec<Uuid>` |
| `connection_groups_list` | — | `Vec<ConnectionGroup>` |
| `connection_groups_create` | `ConnectionGroupInput` | `ConnectionGroup` |
| `connection_groups_update` | `id: Uuid`, `ConnectionGroupInput` | `ConnectionGroup` |
| `connection_groups_delete` | `id: Uuid` | `()` |
| `connection_groups_reorder` | `Vec<{id, parent_id, position}>` | `()` |

### 6.2 `query::*`

| Command | Inputs | Returns |
|---|---|---|
| `query_execute` | `connection_id: Uuid`, `sql: String`, `page: u32`, `page_size: u32` | `QueryResult` |
| `query_execute_selection` | `connection_id: Uuid`, `sql: String` | `QueryResult` |
| `query_cancel` | `query_id: Uuid` | `()` |
| `query_explain` | `connection_id: Uuid`, `sql: String` | `ExplainResult` |
| `query_format` | `sql: String`, `dialect: SqlDialect` | `String` |
| `query_update_rows` | `connection_id: Uuid`, `table: String`, `changes: Vec<RowChange>` | `UpdateResult` |

`QueryResult` contains: `columns: Vec<ColumnMeta>`, `rows: Vec<Vec<serde_json::Value>>`, `total_rows: Option<u64>`, `duration_ms: u64`, `query_id: Uuid`.

### 6.3 `schema::*`

| Command | Inputs | Returns |
|---|---|---|
| `schema_list_databases` | `connection_id: Uuid` | `Vec<String>` |
| `schema_list_tables` | `connection_id: Uuid`, `database: String` | `Vec<TableInfo>` |
| `schema_list_columns` | `connection_id: Uuid`, `database: String`, `table: String` | `Vec<ColumnInfo>` |
| `schema_list_indexes` | `connection_id: Uuid`, `database: String`, `table: String` | `Vec<IndexInfo>` |
| `schema_list_foreign_keys` | `connection_id: Uuid`, `database: String`, `table: String` | `Vec<ForeignKeyInfo>` |
| `schema_list_views` | `connection_id: Uuid`, `database: String` | `Vec<ViewInfo>` |
| `schema_list_functions` | `connection_id: Uuid`, `database: String` | `Vec<RoutineInfo>` |
| `schema_list_procedures` | `connection_id: Uuid`, `database: String` | `Vec<RoutineInfo>` |
| `schema_get_ddl` | `connection_id: Uuid`, `database: String`, `object: String`, `object_type: SchemaObjectType` | `String` |

### 6.4 `history::*`

| Command | Inputs | Returns |
|---|---|---|
| `history_list` | `connection_id: Uuid`, `limit: u32`, `offset: u32` | `Vec<QueryHistoryEntry>` |
| `history_search` | `connection_id: Uuid`, `query: String` | `Vec<QueryHistoryEntry>` |
| `history_clear` | `connection_id: Uuid` | `()` |
| `history_delete` | `id: Uuid` | `()` |

### 6.5 `saved_queries::*`

| Command | Inputs | Returns |
|---|---|---|
| `saved_queries_list_folders` | — | `Vec<SavedQueryFolder>` |
| `saved_queries_create_folder` | `SavedQueryFolderInput` | `SavedQueryFolder` |
| `saved_queries_update_folder` | `id: Uuid`, `SavedQueryFolderInput` | `SavedQueryFolder` |
| `saved_queries_delete_folder` | `id: Uuid` | `()` |
| `saved_queries_list` | `folder_id: Option<Uuid>` | `Vec<SavedQuery>` |
| `saved_queries_create` | `SavedQueryInput` | `SavedQuery` |
| `saved_queries_update` | `id: Uuid`, `SavedQueryInput` | `SavedQuery` |
| `saved_queries_delete` | `id: Uuid` | `()` |

### 6.6 `settings::*`

| Command | Inputs | Returns |
|---|---|---|
| `settings_get` | `key: String` | `serde_json::Value` |
| `settings_get_all` | — | `HashMap<String, serde_json::Value>` |
| `settings_set` | `key: String`, `value: serde_json::Value` | `()` |
| `settings_reset` | `key: String` | `()` |

### 6.7 `keychain::*`

| Command | Inputs | Returns |
|---|---|---|
| `keychain_store` | `connection_id: Uuid`, `secret_type: SecretType`, `value: String` | `()` |
| `keychain_retrieve` | `connection_id: Uuid`, `secret_type: SecretType` | `Option<String>` |
| `keychain_delete` | `connection_id: Uuid`, `secret_type: SecretType` | `()` |

`SecretType` is an enum: `DbPassword`, `SshPassword`, `SshKeyPassphrase`.

### 6.8 `ssh::*`

| Command | Inputs | Returns |
|---|---|---|
| `ssh_create_tunnel` | `connection_id: Uuid` | `u16` (local forwarded port) |
| `ssh_destroy_tunnel` | `connection_id: Uuid` | `()` |
| `ssh_tunnel_status` | `connection_id: Uuid` | `TunnelStatus` |

### 6.9 `erd::*`

| Command | Inputs | Returns |
|---|---|---|
| `erd_get_graph` | `connection_id: Uuid`, `database: String` | `ErdGraph` |

`ErdGraph` contains `nodes: Vec<ErdTable>` (with columns) and `edges: Vec<ErdRelation>` (FK links). Layout is computed in the frontend via elkjs.

### 6.10 `import::*`

| Command | Inputs | Returns |
|---|---|---|
| `import_csv_preview` | `file_path: String` | `CsvPreview` (first N rows + inferred schema) |
| `import_csv_execute` | `connection_id: Uuid`, `ImportCsvOptions` | `ImportResult` |
| `import_sql_file` | `connection_id: Uuid`, `file_path: String` | Stream of `ImportProgress` events |

### 6.11 `export::*`

| Command | Inputs | Returns |
|---|---|---|
| `export_result_to_file` | `ExportRequest`, `format: ExportFormat`, `file_path: String` | `()` |
| `export_result_to_clipboard` | `ExportRequest`, `format: ExportFormat` | `()` |

`ExportFormat`: `Csv`, `Json`, `SqlInsert`, `SqlInClause`, `TabSeparated`.

### 6.12 `themes::*`

| Command | Inputs | Returns |
|---|---|---|
| `themes_list` | — | `Vec<ThemeMeta>` |
| `themes_read` | `name: String` | `ThemeData` |
| `themes_write` | `name: String`, `ThemeData` | `()` |
| `themes_delete` | `name: String` | `()` |
| `themes_duplicate` | `source: String`, `new_name: String` | `ThemeMeta` |

### 6.13 `updater::*`

| Command | Inputs | Returns |
|---|---|---|
| `updater_check` | — | `UpdateCheckResult` |
| `updater_install` | — | `()` |

---

## 7. Rust Backend Design

### 7.1 State Management

Tauri's managed state (`app.manage()`) holds the following application-wide singletons:

```rust
pub struct AppState {
    pub sqlite: SqlitePool,                        // local config DB
    pub connections: Arc<ConnectionManager>,       // active remote pools
    pub ssh_tunnels: Arc<SshTunnelManager>,        // active tunnels
}

pub struct ConnectionManager {
    pools: DashMap<Uuid, RemotePool>,
}

pub enum RemotePool {
    MySql(sqlx::MySqlPool),
    Postgres(sqlx::PgPool),
}
```

### 7.2 Connection Lifecycle

```
connect(id)
  ├── 1. Load profile from SQLite
  ├── 2. Retrieve password from keychain
  ├── 3. If ssh_enabled: ssh::create_tunnel(id) → local_port
  ├── 4. Build connection string (use local_port if SSH)
  ├── 5. Build sqlx PoolOptions with pool_min/pool_max
  ├── 6. Call .connect() — returns RemotePool
  ├── 7. Insert into ConnectionManager.pools
  └── 8. Emit "connection_status_changed" event to frontend

disconnect(id)
  ├── 1. Remove from ConnectionManager.pools (pool.close() called on drop)
  └── 2. If ssh_enabled: ssh::destroy_tunnel(id)
```

Auto-reconnect is handled by `sqlx`'s built-in pool reconnect logic. For connection-level failures detected at query time, the command handler catches the error, attempts one reconnect cycle with exponential backoff (max 3 attempts), and only surfaces the error to the frontend if all attempts fail.

### 7.3 Query Execution

```rust
// query.rs (simplified)
pub async fn query_execute(
    state: State<AppState>,
    connection_id: Uuid,
    sql: String,
    page: u32,
    page_size: u32,
) -> Result<QueryResult, AppError> {
    let pool = state.connections.get(connection_id)?;

    // Check read-only mode
    if profile.read_only && is_mutating_statement(&sql) {
        return Err(AppError::read_only_violation());
    }

    let start = Instant::now();
    let query_id = Uuid::new_v4();

    // Execute via dialect-aware runner
    let raw = match &pool {
        RemotePool::MySql(p) => mysql::execute_query(p, &sql, page, page_size).await,
        RemotePool::Postgres(p) => postgres::execute_query(p, &sql, page, page_size).await,
    }?;

    // Write to query_history
    history::record(&state.sqlite, &connection_id, &sql, start.elapsed(), &raw).await?;

    Ok(raw)
}
```

Multi-statement queries are split on `;` (respecting string literals and comments) in the `utils::sql` module, then executed in sequence. Each statement's result is returned. The frontend renders each result independently.

### 7.4 Schema Introspection

Introspection is dialect-specific. Each `mysql.rs` and `postgres.rs` module provides identical function signatures but different SQL:

- `list_tables(pool, database)` — `information_schema.tables`
- `list_columns(pool, database, table)` — `information_schema.columns`
- `list_indexes(pool, database, table)` — dialect-specific system tables
- `list_foreign_keys(pool, database, table)` — `information_schema.key_column_usage` / `pg_constraint`
- `get_ddl(pool, database, object, type)` — `SHOW CREATE TABLE` for MySQL, `pg_get_tabledef` pattern for Postgres

Schema data is cached in the frontend store after first load and invalidated on user request or reconnect.

### 7.5 Read-Only Enforcement

```rust
fn is_mutating_statement(sql: &str) -> bool {
    let normalized = sql.trim().to_uppercase();
    matches!(normalized.split_whitespace().next(),
        Some("INSERT" | "UPDATE" | "DELETE" | "DROP" | "CREATE" |
             "ALTER" | "TRUNCATE" | "REPLACE" | "MERGE"))
}
```

This check runs in Rust before any query is sent, regardless of what the frontend sends.

### 7.6 SSH Tunnel Lifecycle

```
SshTunnelManager maintains: DashMap<Uuid, TunnelHandle>

TunnelHandle {
    local_port: u16,          // randomly selected free port
    task: JoinHandle<()>,     // tokio task running the forward loop
    shutdown: Sender<()>,     // oneshot to signal shutdown
}

create_tunnel(connection_id):
    1. Load SSH config from profile
    2. Retrieve SSH password or load key from keychain/path
    3. Connect russh session to ssh_host:ssh_port
    4. Authenticate (password or pubkey)
    5. Bind local TCP listener on 127.0.0.1:0 (OS assigns port)
    6. Spawn tokio task: accept loop → for each connection →
       open russh channel → forward bytes bidirectionally
    7. Store TunnelHandle, return local_port
```

### 7.7 Error Handling Strategy

- All command functions return `Result<T, AppError>`
- `AppError` implements `serde::Serialize` so Tauri serialises it into the IPC error slot
- `thiserror` drives the typed inner errors; `anyhow` is used only within implementation functions, converted to `AppError` at command boundaries
- Every error has a stable `code` string the frontend can match on without string parsing

---

## 8. Frontend Architecture

### 8.1 Svelte 5 Runes Approach

Svelte 5 runes replace the store API for all reactive state. The pattern used throughout:

```typescript
// stores/connections.svelte.ts
let connections = $state<ConnectionProfile[]>([]);
let activeConnectionIds = $state<Set<string>>(new Set());

export function useConnections() {
    return {
        get connections() { return connections; },
        get activeConnectionIds() { return activeConnectionIds; },
        async load() { connections = await invoke('connections_list'); },
        async connect(id: string) { /* ... */ },
    };
}
```

Components import from stores and call functions directly — no Svelte store subscriptions or `$:` reactive declarations.

### 8.2 Panel System

The split panel system (`SplitPanel.svelte`) manages a reactive grid up to 2x2. Panel state is stored in `panels.svelte.ts`:

```typescript
type PanelType =
    | { kind: 'query_editor'; connectionId: string; sql: string; }
    | { kind: 'table_browser'; connectionId: string; database: string; table: string; }
    | { kind: 'erd'; connectionId: string; database: string; }
    | { kind: 'empty' };

type PanelLayout = {
    panels: [PanelState, PanelState?, PanelState?, PanelState?];
    split: 'none' | 'horizontal' | 'vertical' | 'quad';
    focusedIndex: number;
};
```

Panels are split by inserting into the array and updating `split`. The `SplitPanel` component renders a CSS grid that reacts to `split`:

- `none` → single cell fills area
- `horizontal` → two columns (50/50, draggable divider)
- `vertical` → two rows
- `quad` → 2×2 grid

Each `Panel.svelte` uses the `PanelType.kind` to switch which component to render inside it.

### 8.3 Tauri Invoke Wrappers

All IPC calls go through typed wrappers in `src/lib/tauri/`. This isolates the raw `invoke` string from the rest of the codebase and allows TypeScript inference:

```typescript
// tauri/query.ts
import { invoke } from '@tauri-apps/api/core';
import type { QueryResult, AppError } from '../types';

export async function executeQuery(
    connectionId: string,
    sql: string,
    page: number,
    pageSize: number,
): Promise<QueryResult> {
    return invoke<QueryResult>('query_execute', { connectionId, sql, page, pageSize });
}
```

### 8.4 CodeMirror Integration

`QueryEditor.svelte` wraps CodeMirror 6 as a Svelte component. Key integration points:

- Editor view created in `onMount`, destroyed in `onDestroy`
- SQL dialect set from the active connection's `db_type`
- Autocomplete source is a custom `CompletionSource` that reads from the schema store
- Keyboard shortcuts (Run All, Run Selection, etc.) are bound via `keymap.of([...])` using the effective shortcut map from `shortcuts.svelte.ts`
- The editor emits a Svelte custom event on execution, which `ResultsPanel` listens to

Schema-aware autocomplete flow:
1. On connection, `schema_list_tables` and `schema_list_columns` are fetched and stored
2. The CodeMirror completion source consults this cached data
3. Completions include table names at the top level, then column names contextually after `table.` or in `SELECT`/`WHERE` positions

### 8.5 DataTable (TanStack Table)

`DataTable.svelte` wraps `@tanstack/svelte-table`. Configuration:

- `getCoreRowModel`, `getSortedRowModel`, `getFilteredRowModel`, `getPaginationRowModel` — all enabled
- Column sizing via `columnResizeMode: 'onChange'`
- Column order maintained in component state, synced to a per-table preference in settings
- Filter row rendered as a `<tr>` below the header row with `<input>` per column
- Row selection tracked with `getSelectionRowModel`

Cell editing:

```
User double-clicks a cell
  → CellEditor mounts over the cell (position: absolute)
  → Shows appropriate control for the column type
  → On confirm: queues change in local `pendingChanges: Map<rowKey, Map<colName, newValue>>`
  → "Save Changes" button becomes visible
  → On save: calls query_update_rows with the pending changes
  → Backend constructs parameterised UPDATE statements
  → On success: refreshes the result set
```

### 8.6 ERD Canvas

`ErdCanvas.svelte` renders the ERD as a `<svg>` element with:

1. Fetch `erd_get_graph(connectionId, database)` — returns nodes and edges
2. Build elkjs graph input (`ELK.layout(graph)`) in a Web Worker (via `web-worker` package) to avoid blocking the UI thread
3. elkjs returns `x, y, width, height` for each node and bend-points for each edge
4. Render as Svelte-reactive SVG: `<g>` per table, `<path>` per FK relation
5. `d3-zoom` manages the SVG `viewBox` transform for pan/zoom
6. Click on a table node dispatches an event to open the table in a main area panel

Edge routing uses elkjs `org.eclipse.elk.edgeRouting: ORTHOGONAL` for clean right-angle lines.

---

## 9. Feature Breakdown

### 9.1 Connection Management

**Create/Edit Connection Form** (multi-step or tabbed):
- Tab 1 — Basic: name, group, type, host, port, database, username, color picker
- Tab 2 — SSH Tunnel: enable toggle, host, port, user, auth type (password/key), key file picker
- Tab 3 — SSL/TLS: enable toggle, CA cert path, client cert path, client key path
- Tab 4 — Advanced: read-only toggle, pool min/max, connection timeout

Password fields never leave the frontend as plaintext after the initial entry — they are passed directly to `keychain_store` and never stored in SQLite or in any Svelte state after that point.

**Connection Tree** (left sidebar):
- Groups rendered as collapsible folders (drag to reorder)
- Connection items show color swatch and name
- Right-click context menu: Connect, Edit, Duplicate, Delete, Copy connection string
- Connected indicator (dot) per item
- Drag connections between groups

### 9.2 Query Editor

**Statement Detection** (cursor-based execution):
```
Given a multi-statement SQL document, the "run under cursor" action must:
1. Find the cursor position
2. Walk backwards to find the start of the current statement
   (line before a semicolon, or document start)
3. Walk forwards to find the end (the next semicolon, or document end)
4. Execute that substring
```

This logic lives in `src/lib/utils/sql.ts` as a pure function, independently testable.

**Format Query**: calls `sql-formatter` directly in the frontend (pure JS function, no round-trip needed), then updates the CodeMirror document with the formatted SQL.

### 9.3 Results Panel and Table Browser

**Pagination**: The backend returns `total_rows` from a `COUNT(*)` sub-query run alongside the main query (for SELECTs). Page controls appear at the bottom. Changing page triggers a new `query_execute` call with updated `page`/`page_size`.

**Column Inspector (right sidebar)**: when a column is clicked in the results table or schema tree, the right sidebar's Column Inspector panel shows:
- Column name, data type, nullable, default value
- Whether it is a primary key, unique, or foreign key
- Value distribution (top N values from a sampled `GROUP BY` query) — fetched lazily on request, not automatically

**Export options**:

| Format | Description |
|---|---|
| CSV | Comma-separated, quoted where needed, with header row |
| JSON | Array of objects keyed by column name |
| SQL INSERT | `INSERT INTO table_name (cols) VALUES (...), (...);` |
| SQL IN clause | `(val1, val2, val3, ...)` for pasting into `WHERE id IN (...)` |
| Tab-separated | Default clipboard copy, pastes cleanly into spreadsheets |

### 9.4 Schema Tree

The schema tree is a recursive tree structure:

```
Connection
└── Database
    ├── Tables
    │   └── table_name
    │       ├── Columns (col_name type nullable)
    │       ├── Indexes
    │       └── Foreign Keys
    ├── Views
    ├── Functions
    └── Stored Procedures
```

Fuse.js search is applied to the flattened list of node names. The search input at the top of the sidebar filters nodes by matching name, with matching nodes' parents expanded automatically.

Context menu actions per node type:

| Node Type | Context Menu Options |
|---|---|
| Table | Open Table, Open in New Panel, View DDL, Open in ERD, Copy Name |
| View | Open View, View DDL, Copy Name |
| Column | Copy Name, Copy as SQL Reference |
| Database | Refresh Schema, Open ERD |

### 9.5 EXPLAIN Visualisation

The EXPLAIN visualiser runs the appropriate command per dialect:
- MySQL/MariaDB: `EXPLAIN FORMAT=JSON <query>` or `EXPLAIN ANALYZE <query>` (MariaDB 10.9+)
- PostgreSQL: `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) <query>`

The JSON output is parsed into a tree of nodes. Each node is rendered as a card showing:
- Node type (e.g. `Seq Scan`, `Hash Join`, `Index Scan`)
- Table name (if applicable)
- Estimated and actual rows
- Estimated and actual cost/time
- Buffers hit/read (Postgres)

Nodes are laid out with elkjs (same pattern as ERD). High-cost nodes are highlighted with a warning colour (CSS variable `--color-warning`).

### 9.6 Import

**CSV Import flow**:
1. User opens file picker (native dialog via `tauri-plugin-dialog`)
2. Backend reads first 20 rows and infers column types (integer, float, text, date)
3. Preview shown in a modal with editable column names and type overrides
4. User chooses: "Create new table" (name input) or "Append to existing table" (table picker)
5. On confirm: backend streams INSERT batches (1000 rows at a time), progress shown as a bar

**SQL File Import flow**:
1. User opens `.sql` file
2. Backend streams the file, splitting on `;` with comment and string-literal awareness
3. Executes each statement sequentially, emitting Tauri events for progress
4. Frontend shows a progress modal with statement count, current statement preview, and any errors encountered
5. User can choose to stop on first error or continue

---

## 10. Layout System

### 10.1 Sidebar Resize

Both sidebars use pointer-event drag listeners. The pattern:

- A draggable divider element (`<div class="resize-handle">`) positioned between panes
- `pointerdown` → capture pointer → track `pointermove` → update CSS variable `--sidebar-width`
- Persist width to settings on `pointerup`
- Min/max clamp applied (e.g., left sidebar: 160px–500px)

The left sidebar's top/bottom split is the same mechanism but vertical, controlling `--sidebar-top-height` as a percentage.

### 10.2 Panel Layout State Machine

```
States: single | split-h | split-v | quad

Transitions:
  single       + "split right"  → split-h   (panel[0] left, panel[1] right)
  single       + "split down"   → split-v   (panel[0] top,  panel[1] bottom)
  split-h      + "split down"   → quad      (4 panels)
  split-v      + "split right"  → quad      (4 panels)
  quad         + "close panel"  → split-h/v (remove panel, merge cells)
  split-h/v    + "close panel"  → single
```

Panel focus is tracked; keyboard shortcut "next/prev panel" cycles through the active panels.

### 10.3 Right Sidebar Tabs

The right sidebar has four possible panels shown as icon tabs:

| Icon | Panel | Description |
|---|---|---|
| Column icon | Column Inspector | Details for selected column |
| Clock icon | Query History | Per-connection history list |
| Bookmark icon | Saved Queries | Folder tree of saved queries |
| Info icon | Table Info | Row count, size, create time |

Only one panel is visible at a time. Clicking an active tab collapses the sidebar entirely.

---

## 11. Theme System

### 11.1 CSS Variable Architecture

Every visual token is a CSS variable defined in `src/styles/variables.css`. The variable naming follows a semantic pattern:

```css
/* variables.css — defaults (light theme values) */
:root {
    /* Colour palette */
    --color-bg-primary: #ffffff;
    --color-bg-secondary: #f5f5f5;
    --color-bg-tertiary: #ebebeb;
    --color-bg-overlay: #ffffff;
    --color-border: #e0e0e0;
    --color-border-strong: #c0c0c0;
    --color-text-primary: #1a1a1a;
    --color-text-secondary: #666666;
    --color-text-muted: #999999;
    --color-accent: #4f46e5;
    --color-accent-hover: #4338ca;
    --color-accent-text: #ffffff;
    --color-danger: #dc2626;
    --color-warning: #d97706;
    --color-success: #16a34a;

    /* Editor */
    --color-editor-bg: #fafafa;
    --color-editor-text: #1a1a1a;
    --color-editor-selection: #b3d4fc;
    --color-editor-active-line: #f0f0f0;
    --color-editor-keyword: #7c3aed;
    --color-editor-string: #16a34a;
    --color-editor-number: #b45309;
    --color-editor-comment: #9ca3af;

    /* Spacing */
    --spacing-xs: 4px;
    --spacing-sm: 8px;
    --spacing-md: 12px;
    --spacing-lg: 16px;
    --spacing-xl: 24px;

    /* Typography */
    --font-family-ui: system-ui, -apple-system, sans-serif;
    --font-family-mono: ui-monospace, 'Cascadia Code', 'Fira Code', monospace;
    --font-size-sm: 11px;
    --font-size-md: 13px;
    --font-size-lg: 15px;

    /* Radius */
    --radius-sm: 3px;
    --radius-md: 5px;
    --radius-lg: 8px;

    /* Shadows */
    --shadow-sm: 0 1px 2px rgba(0,0,0,0.06);
    --shadow-md: 0 2px 8px rgba(0,0,0,0.12);
    --shadow-lg: 0 4px 24px rgba(0,0,0,0.16);

    /* Transitions */
    --transition-fast: 100ms ease;
    --transition-md: 200ms ease;

    /* Layout */
    --sidebar-width: 240px;
    --right-sidebar-width: 280px;
    --sidebar-top-height: 40%;
    --titlebar-height: 36px;
    --toolbar-height: 36px;
}
```

Dark theme overrides only the values that differ:

```css
/* themes/dark.css */
[data-theme="dark"] {
    --color-bg-primary: #1a1a1a;
    --color-bg-secondary: #242424;
    /* ... */
}

@media (prefers-color-scheme: dark) {
    [data-theme="system"] {
        --color-bg-primary: #1a1a1a;
        /* ... same overrides ... */
    }
}
```

The root element gets `data-theme="light|dark|system"` from the settings store on startup.

### 11.2 User Theme Files

Theme files live at `~/.config/rowmance/themes/<name>.json`:

```json
{
    "name": "Dracula",
    "extends": "dark",
    "variables": {
        "--color-bg-primary": "#282a36",
        "--color-accent": "#bd93f9",
        "--color-editor-keyword": "#ff79c6"
    }
}
```

On load, the backend reads the file and the frontend injects the variables as inline `style` overrides on `:root`. The `extends` field determines which built-in theme is the base.

### 11.3 In-App Theme Editor

`ThemeEditor.svelte` renders a grouped list of all CSS variables (grouped by category: Colours, Editor, Typography, Spacing). Each row shows:
- Variable name
- Current value (colour swatch for colours, text for others)
- A colour picker (for colour variables) or text input
- A "reset to base" button

Changes apply immediately to the DOM (for live preview) and are saved to the theme file via `themes_write` on a debounced 500ms timer.

---

## 12. Keyboard Shortcut System

### 12.1 Action Registry

All bindable actions are defined as constants in `src/lib/utils/shortcuts.ts`:

```typescript
export const ACTIONS = {
    // Query execution
    QUERY_RUN_ALL: 'query.run_all',
    QUERY_RUN_SELECTION: 'query.run_selection',
    QUERY_RUN_UNDER_CURSOR: 'query.run_under_cursor',
    QUERY_FORMAT: 'query.format',
    QUERY_EXPLAIN: 'query.explain',
    // Navigation
    FOCUS_EDITOR: 'focus.editor',
    FOCUS_SCHEMA_TREE: 'focus.schema_tree',
    FOCUS_RESULTS: 'focus.results',
    PANEL_NEXT: 'panel.next',
    PANEL_PREV: 'panel.prev',
    PANEL_CLOSE: 'panel.close',
    TOGGLE_RIGHT_SIDEBAR: 'sidebar.toggle_right',
    // Editing
    TABLE_SAVE_CHANGES: 'table.save_changes',
    TABLE_DISCARD_CHANGES: 'table.discard_changes',
    // Pagination
    PAGE_NEXT: 'pagination.next',
    PAGE_PREV: 'pagination.prev',
    // Global
    COMMAND_PALETTE: 'global.command_palette',
    NEW_QUERY_EDITOR: 'global.new_query',
} as const;
```

### 12.2 Preset Definitions

Each preset is a `Record<ActionId, string>` where the shortcut string uses the format `"Meta+Enter"`, `"Ctrl+Shift+Enter"`, etc. (`Meta` = Cmd on macOS, Windows key on Windows):

| Action | VS Code (default) | JetBrains | Vim |
|---|---|---|---|
| Run all query | `Meta+Enter` | `Meta+Enter` | `<leader>r` |
| Run selection | `Meta+Shift+Enter` | `Meta+Shift+Enter` | `<leader>s` |
| Run under cursor | `Alt+Enter` | `Ctrl+Enter` | `<leader>c` |
| Format query | `Shift+Alt+F` | `Ctrl+Alt+L` | `<leader>f` |
| Command palette | `Meta+K` | `Shift+Shift` | `:` |
| New query editor | `Meta+N` | `Meta+N` | `<leader>n` |
| Toggle right sidebar | `Meta+B` | `Meta+B` | `<leader>b` |
| Save changes | `Meta+S` | `Meta+S` | `:w` |

### 12.3 Runtime Shortcut Resolution

The effective shortcut map is computed in `shortcuts.svelte.ts`:

```typescript
// Merge order: preset base → per-action overrides from SQLite
const effectiveShortcuts = $derived(() => {
    const base = PRESETS[settings.shortcut_preset];
    const overrides = shortcutOverrides; // from SQLite
    return { ...base, ...overrides };
});
```

A global `keydown` listener on `window` checks the event against the effective map and dispatches a custom event with the action name. Components listen for these events on relevant elements, not for raw key combos.

---

## 13. Development Standards

### 13.1 Code Style

**General principles across both Rust and TypeScript/Svelte:**
- Use clear, descriptive variable and function names — prefer `connectionProfileId` over `id`, `pendingRowChanges` over `changes`
- Break functionality into many small, single-responsibility functions and components; no function should do more than one thing
- Prefer OOP-style organisation: group related state and behaviour into classes or well-structured modules rather than scattering logic across files
- Write comments that explain *why*, not *what* — the what should be clear from the name; the why often isn't
- Every public function, struct, and component should have a short doc comment

**Frontend (TypeScript/Svelte):**
- All Svelte components should be small and focused — if a component is growing large, extract sub-components
- Use TypeScript strictly (`strict: true` in `tsconfig.json`); no `any` types
- Name Svelte components in PascalCase, utilities and stores in camelCase
- Prefer named exports over default exports for utilities and stores
- CSS is scoped per-component; use CSS variables for all values that could be themed

**Rust:**
- Follow standard Rust naming conventions: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants
- Keep command handler functions thin — they validate inputs and delegate to domain modules; business logic lives in `connections/`, `db/`, etc.
- Every `pub` function and type in a command module must have a `///` doc comment
- Prefer `thiserror`-derived error types over `anyhow` at module boundaries; use `anyhow` only inside implementation functions

### 13.2 Testing Strategy

Tests are written **alongside implementation** — every feature added in a phase is tested in the same phase, not deferred to a polish pass. The goal is comprehensive coverage by v1.0.

#### Frontend Tests

Use **Vitest** for unit and integration tests, **Svelte Testing Library** (`@testing-library/svelte`) for component tests.

| Test type | What to cover | Location |
|---|---|---|
| Unit | Pure utility functions (`sql.ts` splitter, `shortcuts.ts` normaliser, `format.ts`) | `src/lib/utils/*.test.ts` |
| Unit | Tauri invoke wrappers (mock `invoke`, assert correct command name and payload shape) | `src/lib/tauri/*.test.ts` |
| Unit | Store logic (panel state transitions, shortcut resolution, settings merging) | `src/lib/stores/*.test.ts` |
| Component | Key components in isolation (DataTable pagination, CellEditor controls, SchemaTree filtering) | `src/lib/components/**/*.test.ts` |
| Integration | Full user flows mocked at the Tauri boundary (connect → query → edit → save) | `src/tests/` |

**Coverage target**: 80% line coverage minimum on `src/lib/utils/` and `src/lib/stores/`; component tests for every interactive component.

Critical units to test thoroughly:
- `sql.ts` — statement splitter (multi-statement, string literals, comments, edge cases)
- `shortcuts.ts` — preset merging, override application, key normalisation across platforms
- Panel state machine transitions (all valid and invalid transitions)
- `is_mutating_statement` (mirrored in frontend for UI hints)

#### Rust Tests

Use Rust's built-in `#[test]` and `#[tokio::test]` for async tests. Use `sqlx::test` macro for database integration tests (spins up a real SQLite/Postgres/MySQL instance).

| Test type | What to cover | Location |
|---|---|---|
| Unit | Pure logic: `is_mutating_statement`, SQL statement splitter, CSV type inference, pagination offset calculation | `src-tauri/src/*/tests.rs` or inline `#[cfg(test)]` blocks |
| Integration | Tauri command handlers against a real SQLite database (connection CRUD, settings, history, saved queries) | `src-tauri/tests/` |
| Integration | Schema introspection queries against MySQL and Postgres test containers (via `testcontainers` crate) | `src-tauri/tests/integration/` |
| Integration | SSH tunnel establishment and teardown (use a local `openssh` mock or test container) | `src-tauri/tests/integration/` |

Add the following to `Cargo.toml`:
```toml
[dev-dependencies]
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["mysql", "postgres"] }
tokio = { version = "1", features = ["full"] }
```

**Coverage target**: All Tauri command handlers have at least one happy-path integration test. All pure functions have unit tests covering edge cases.

### 13.3 Linting and Formatting Tools

All tools run in CI and are expected to pass with zero warnings before any code is merged.

#### Frontend

| Tool | Purpose | Config file | Run command |
|---|---|---|---|
| **Prettier** | Code formatting (TypeScript, Svelte, CSS) | `.prettierrc` | `bunx prettier --check src/` |
| **ESLint** | Linting (TypeScript rules + Svelte plugin) | `eslint.config.js` | `bunx eslint src/` |
| **tsc** | TypeScript type checking | `tsconfig.json` | `bunx tsc --noEmit` |
| **svelte-check** | Svelte-specific type and a11y checks | — | `bunx svelte-check` |
| **Vitest** | Test runner | `vite.config.ts` | `bun run test` |

Recommended `.prettierrc`:
```json
{
  "semi": true,
  "singleQuote": true,
  "trailingComma": "all",
  "printWidth": 100,
  "tabWidth": 2,
  "plugins": ["prettier-plugin-svelte"],
  "overrides": [{ "files": "*.svelte", "options": { "parser": "svelte" } }]
}
```

Add to `package.json` scripts:
```json
{
  "scripts": {
    "dev": "tauri dev",
    "build": "tauri build",
    "check": "bunx svelte-check && bunx tsc --noEmit",
    "lint": "bunx eslint src/ && bunx prettier --check src/",
    "format": "bunx prettier --write src/",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:coverage": "vitest run --coverage"
  }
}
```

#### Rust

| Tool | Purpose | Run command |
|---|---|---|
| **rustfmt** | Code formatting | `cargo fmt --check` (CI) / `cargo fmt` (local) |
| **Clippy** | Linting with Rust best-practice rules | `cargo clippy -- -D warnings` |
| **cargo test** | Test runner | `cargo test` |
| **cargo doc** | Documentation build check | `cargo doc --no-deps` |

Add a `rustfmt.toml` at the repo root:
```toml
edition = "2021"
max_width = 100
use_small_heuristics = "Default"
```

Clippy should run with `-D warnings` (treat all warnings as errors). Add a `.cargo/config.toml` to enforce this in CI:
```toml
[target.'cfg(all())']
rustflags = ["-D", "warnings"]
```

### 13.4 README Maintenance

`README.md` is a living document updated throughout development, not written once at the end. It should always reflect the current state of the project.

**README must include at all times:**
- Project description and screenshot (update the screenshot whenever the UI changes significantly)
- Prerequisites (Rust toolchain version, bun version, OS requirements)
- Quick-start instructions (`git clone` → `make install` → `make dev`)
- Full `make` target reference (generated or kept in sync with the Makefile)
- Configuration file locations (`~/.config/rowmance/`)
- Supported database versions (MySQL 5.7+, MariaDB 10.5+, PostgreSQL 13+)
- Link to `CONTRIBUTING.md`

**Update the README when:**
- A new phase is completed or a major feature is added
- Prerequisites change (new tool, version bump)
- The Makefile gains or loses targets
- Configuration options change
- The UI changes enough that the screenshot is misleading

### 13.5 CI Pipeline

Use GitHub Actions. Every push to any branch runs the full check suite; merging to `main` additionally runs tests.

```yaml
# .github/workflows/ci.yml (outline)
jobs:
  frontend:
    - bun install
    - bunx svelte-check
    - bunx tsc --noEmit
    - bunx eslint src/
    - bunx prettier --check src/
    - bun run test:coverage

  rust:
    - cargo fmt --check
    - cargo clippy -- -D warnings
    - cargo test
    - cargo doc --no-deps
```

---

## 14. Makefile

A `Makefile` lives at the repo root and provides a single entry point for all common development tasks. Use `make help` to list available targets. All targets that wrap `bun` or `cargo` commands should print what they are running before executing.

```makefile
# Rowmance — developer convenience targets
# Requires: bun, cargo, rustup (with rustfmt + clippy components)

.PHONY: help dev build check lint format test test-watch test-coverage \
        rust-check rust-lint rust-test rust-fmt rust-doc \
        clean install update

## Show available targets
help:
	@grep -E '^##' Makefile | sed 's/## //'

## Install all dependencies (frontend + Rust toolchain components)
install:
	bun install
	rustup component add rustfmt clippy

## Start the app in development mode (hot reload)
dev:
	bun run dev

## Build a production release for the current platform
build:
	bun run build

# ── Frontend ─────────────────────────────────────────────────────────────────

## Run all frontend checks (svelte-check, tsc, eslint, prettier)
check:
	bunx svelte-check
	bunx tsc --noEmit
	bunx eslint src/
	bunx prettier --check src/

## Run ESLint and Prettier checks
lint:
	bunx eslint src/
	bunx prettier --check src/

## Auto-fix formatting with Prettier
format:
	bunx prettier --write src/

## Run frontend tests once
test:
	bun run test

## Run frontend tests in watch mode
test-watch:
	bun run test:watch

## Run frontend tests with coverage report
test-coverage:
	bun run test:coverage

# ── Rust ─────────────────────────────────────────────────────────────────────

## Run rustfmt check, clippy, and cargo test
rust-check:
	cargo fmt --check --manifest-path src-tauri/Cargo.toml
	cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
	cargo test --manifest-path src-tauri/Cargo.toml

## Run Clippy linter (warnings treated as errors)
rust-lint:
	cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

## Run Rust test suite
rust-test:
	cargo test --manifest-path src-tauri/Cargo.toml

## Auto-fix Rust formatting
rust-fmt:
	cargo fmt --manifest-path src-tauri/Cargo.toml

## Build and verify Rust documentation
rust-doc:
	cargo doc --manifest-path src-tauri/Cargo.toml --no-deps

# ── Combined ─────────────────────────────────────────────────────────────────

## Run all checks — frontend and Rust (CI equivalent)
ci: check rust-check

## Remove build artefacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	rm -rf node_modules .svelte-kit src-tauri/target

## Update all dependencies
update:
	bun update
	cargo update --manifest-path src-tauri/Cargo.toml
```

---

## 15. Phased Implementation Plan

### Phase 1 — Foundation (Weeks 1–3)

**Goal**: Runnable app with basic connection and query execution.

#### Week 1: Scaffold and Layout

- [x] `bun create tauri-app rowmance` with Svelte 5 template
- [x] Configure `bun.lockb`, `tsconfig.json`, `vite.config.ts`
- [x] Add all Rust crates to `Cargo.toml`
- [x] Add all npm packages to `package.json`
- [x] Implement static three-column layout (left sidebar, main area, right sidebar) with resize handles
- [x] CSS variables system (`variables.css`, `light.css`, `dark.css`)
- [x] Apply `data-theme` from `localStorage` before first render to avoid flash

#### Week 2: SQLite and Connection Profiles

- [x] SQLite pool initialisation in `db/mod.rs`
- [x] Migration runner pointing at `db/migrations/0001_init.sql`
- [x] `AppState` setup and `app.manage()` in `lib.rs`
- [x] Implement `connections::*` commands (CRUD only, no keychain yet)
- [x] `ConnectionForm.svelte` (basic fields, no SSH/SSL tabs yet)
- [x] `ConnectionTree.svelte` (flat list for now, no groups)
- [x] Persist and load connections on startup

#### Week 3: Query Execution

- [x] `ConnectionManager` with `DashMap<Uuid, RemotePool>`
- [x] `connections_connect` / `connections_disconnect` commands
- [x] `query_execute` command (MySQL + Postgres, full page + page_size support)
- [x] Read-only enforcement in Rust
- [x] `QueryEditor.svelte` — CodeMirror 6, no autocomplete, SQL highlighting only
- [x] `DataTable.svelte` — TanStack Table, read-only, basic pagination
- [x] `ResultsPanel.svelte` — editor above, results below, run on Cmd+Enter
- [x] `SchemaTree.svelte` — static tree rendering, no search, no context menu yet
- [x] Query history write on every execution

**Phase 1 Deliverable**: User can add a connection, connect to MySQL or Postgres, write SQL, run it, and see paginated results.

---

### Phase 2 — Core UX (Weeks 4–6)

**Goal**: Feature-complete for daily use; professional-feeling UX.

#### Week 4: Panel System and Table Browser

- [x] `SplitPanel.svelte` — up to 2x2, drag-to-split, drag-to-resize dividers
- [x] `panels.svelte.ts` — panel layout state, focus tracking
- [x] `Panel.svelte` — routes `PanelType` to the correct component
- [x] `SidebarTopHalf.svelte` — list of open panels (click to focus, close button)
- [x] `TableBrowser.svelte` — opens on table click in schema tree, filter bar, sort controls
- [x] Schema tree context menu (right-click): open table, view DDL
- [x] `DdlViewer.svelte` — read-only CodeMirror panel for DDL

#### Week 5: Inline Editing and Column Controls

- [x] `CellEditor.svelte` — text input, Set NULL button, boolean toggle
- [x] Date picker and datetime picker controls in `CellEditor`
- [x] Pending changes queue and "Save Changes" toolbar button
- [x] `query_update_rows` command — parameterised UPDATE construction
- [x] Column resize (TanStack `columnResizeMode`)
- [x] Column reorder (drag column headers)
- [x] `ColumnPicker.svelte` — show/hide columns popover
- [x] Client-side column sorting (click header)
- [x] Column filter row (input below header)

#### Week 6: Schema Features and Sidebar Panels

- [x] Schema-aware autocomplete in CodeMirror (`schema_list_tables`, `schema_list_columns` on connect)
- [x] Fuzzy search on schema tree with fuse.js
- [x] Connection groups/folders in `ConnectionTree.svelte`
- [x] `connection_groups_*` commands
- [x] Query history panel (right sidebar) — clickable, re-opens query in editor
- [x] Saved queries panel (right sidebar) — folder tree, create/edit/delete
- [x] `saved_queries_*` commands

**Phase 2 Deliverable**: Full query-and-edit workflow; schema exploration; query history; saved queries.

---

### Phase 3 — Features (Weeks 7–10)

**Goal**: Production-ready feature set covering security, import/export, and power-user tools.

#### Week 7: Security and Remote Connections

- [x] `keychain_*` commands using `keyring` crate
- [x] Update `ConnectionForm` to pass password to keychain, never SQLite
- [x] Retrieve credential from keychain in `connections_connect`
- [x] SSH tab in `ConnectionForm`
- [x] `ssh_tunnel.rs` — `russh`-based port forwarding
- [x] `ssh_*` commands (`create_tunnel`, `destroy_tunnel`, `tunnel_status`)
- [x] SSL/TLS tab in `ConnectionForm`; pass cert paths to sqlx `ConnectOptions`

#### Week 8: Command Palette and Shortcuts

- [x] `shortcuts.svelte.ts` — preset definitions, override loading, effective map
- [x] `settings_*` commands
- [x] Global keyboard listener + action event dispatch
- [x] `CommandPalette.svelte` — Cmd+K, fuse.js over commands + connections + tables + saved queries
- [x] Shortcut bindings wired into all relevant components
- [x] `KeyboardShortcuts.svelte` settings page — table of all actions, editable bindings
- [x] `keyboard_shortcut_overrides` SQLite read/write

#### Week 9: Export and Import

- [x] `export_*` commands — all formats, to file and clipboard
- [x] Export button and format picker in `ResultsPanel` and `TableBrowser`
- [x] "Copy" context menu on selected rows (Tab-separated default)
- [x] `import_csv_preview` + `import_csv_execute` commands
- [x] CSV import modal (`CsvImportModal.svelte`) — preview, column editor, target picker
- [x] `import_sql_file` command with Tauri event streaming
- [x] SQL import progress modal

#### Week 10: Multi-statement, Format, and Transactions

- [x] Multi-statement SQL splitter in `src/lib/utils/sql.ts` and Rust
- [x] Run selection / run under cursor actions
- [x] `query_format` integration — call `sql-formatter` in frontend, update editor content
- [x] Format shortcut (Shift+Alt+F)
- [x] Transaction control UI (Begin/Commit/Rollback toolbar)
- [x] `settings` transaction mode toggle
- [x] Transaction state indicator in toolbar

**Phase 3 Deliverable**: Secure connections via keychain, SSH, and SSL; full import/export; command palette; all keyboard shortcuts working.

---

### Phase 4 — Advanced (Weeks 11–14)

**Goal**: Differentiated features — ERD, EXPLAIN, themes, auto-update.

#### Week 11: ERD

- [x] `erd_get_graph` command — introspect all FK relationships for a database
- [x] `ErdCanvas.svelte` — elkjs layout in Web Worker, SVG rendering
- [x] Pan (d3-drag) and zoom (d3-zoom) on SVG
- [x] Click table node → open in main panel
- [x] Export PNG (`<canvas>` drawImage from SVG blob URL)
- [x] Export SVG (serialize SVG element)
- [x] "Open in ERD" context menu option in schema tree

#### Week 12: EXPLAIN Visualiser

- [x] `query_explain` command — runs `EXPLAIN (ANALYZE, FORMAT JSON)` per dialect
- [x] EXPLAIN output parser for MySQL JSON format and Postgres JSON format
- [x] `ExplainCanvas.svelte` — elkjs node tree, cost highlighting
- [x] Keyboard shortcut for "Explain current query"
- [x] Display explain result in a main panel

#### Week 13: Theme Editor and Settings Page

- [x] `themes_*` commands — list, read, write, delete, duplicate
- [x] `ThemeEditor.svelte` — grouped variable list, colour picker, live preview, debounced save
- [x] "Duplicate built-in theme" flow
- [x] Theme picker in settings and command palette
- [x] `Settings.svelte` — full settings page with all sections:
  - General, Appearance, Editor, Keyboard, Connections

#### Week 14: Auto-update and Multiple Windows

- [x] `tauri-plugin-updater` integration in `updater.rs`
- [x] `updater_check` / `updater_install` commands
- [x] Update notification banner (dismissible, with "Install now" / "Later" buttons)
- [x] Check on startup if `auto_update_check` setting is true
- [x] Multiple window support: "Open New Window" menu item
- [x] Window state independence (each window has its own panel layout)
- [x] Right sidebar `ColumnInspector` and `TableInfo` panels completed

**Phase 4 Deliverable**: ERD, EXPLAIN, custom themes, auto-update, multiple windows.

---

### Phase 5 — Polish (Weeks 15–16)

**Goal**: Production release quality.

#### Week 15: Keyboard Navigation and Accessibility

- [ ] Audit every interactive element for keyboard reachability (Tab order)
- [ ] Focus trap in modals (dialog, command palette)
- [ ] ARIA roles and labels on custom components (tree, data table, split panels)
- [ ] Screen reader test pass on macOS (VoiceOver)
- [ ] High-contrast theme variant
- [ ] Keyboard navigation within data table (arrow keys, Enter to edit)
- [ ] Skip links where appropriate

#### Week 16: Error Handling, Onboarding, and Docs

- [ ] Connection error UX — inline error with reconnect button
- [ ] Query error UX — error displayed below editor with line number if available
- [ ] Toast/notification system for background operations (import progress, update available)
- [ ] Empty state screen when no connections exist (illustrated prompt to add first connection)
- [ ] Onboarding tooltip sequence for first-time users (dismissible)
- [ ] `README.md` — project overview, build instructions, contribution guide
- [ ] `CONTRIBUTING.md` — branch conventions, PR process, testing guide
- [ ] End-to-end smoke test for core path (connect → query → edit → save)

**Phase 5 Deliverable**: v1.0 release candidate.

---

## 16. Key Design Decisions

### D1: sqlx over Diesel or SeaORM for remote queries

**Decision**: Use `sqlx` for all database access — both local SQLite and remote MySQL/Postgres.

**Rationale**: `sqlx` supports runtime-polymorphic connection via `AnyPool` while still providing async, fully type-checked queries. Diesel is synchronous and requires generated code; SeaORM adds an ORM abstraction layer that would need to be bypassed for raw SQL execution (the primary use case). `sqlx` with `query_as!` macros gives compile-time checked queries for the local SQLite layer, and `query()` with row-level JSON serialisation for the dynamic remote queries.

### D2: JSON rows over typed result rows for remote query results

**Decision**: Remote query results are returned as `Vec<Vec<serde_json::Value>>` rather than strongly-typed structs.

**Rationale**: The schema is not known at compile time — users run arbitrary SQL against databases with arbitrary schemas. Every column value must be serialised to a JSON-compatible type (strings, numbers, booleans, null) for transport over the Tauri IPC. The frontend receives `serde_json::Value` and renders it as a string, with type-specific cell rendering logic based on the column's `data_type` metadata sent alongside the rows.

### D3: elkjs for ERD and EXPLAIN layout, not a graph library

**Decision**: Use elkjs (Eclipse Layout Kernel compiled to JS) for computing node positions, and render with custom SVG in Svelte rather than using a graph library like Cytoscape or React Flow.

**Rationale**: elkjs produces significantly better orthogonal layouts for database schemas (tables with many foreign-key edges) than force-directed layouts. Custom SVG rendering in Svelte keeps the bundle small and gives total control over the visual style using CSS variables. The main cost is the implementation effort for the SVG renderer and interaction layer (pan/zoom via d3), which is manageable.

### D4: No Tailwind — CSS variables only

**Decision**: The entire styling system is built on CSS custom properties. No utility-class framework is used.

**Rationale**: A database client's theme system needs to be user-editable at runtime. With Tailwind, utility classes are fixed at build time and cannot be overridden by user theme files. CSS variables, by contrast, can be reassigned by injecting a `<style>` block at runtime or setting `element.style.setProperty()`, making the in-app theme editor straightforward to implement.

### D5: Credentials never stored in SQLite

**Decision**: Passwords, SSH passwords, and SSH key passphrases are stored exclusively in the OS keychain via the `keyring` crate. The `connection_profiles` table contains no secret fields.

**Rationale**: SQLite at `~/.config/rowmance/rowmance.db` is a plaintext file readable by any process with filesystem access. The OS keychain is encrypted at rest and requires OS-level authentication.

### D6: Svelte 5 runes instead of stores

**Decision**: All reactive state uses Svelte 5 runes (`$state`, `$derived`, `$effect`) in `.svelte.ts` files rather than the Svelte 4 store API.

**Rationale**: Runes are the canonical Svelte 5 approach and provide fine-grained reactivity without the subscription boilerplate of writable/readable stores. The `.svelte.ts` convention makes it explicit that a TypeScript file contains reactive state.

### D7: Row-level pagination for all result sets (no virtual scrolling initially)

**Decision**: Phase 1–4 use server-side (Rust-side) pagination via SQL `LIMIT`/`OFFSET`. Virtual scrolling is deferred to a post-v1 optimisation if performance requires it.

**Rationale**: Pagination with configurable page sizes (up to 500 rows) is simpler to implement correctly than virtual scrolling and is sufficient for most use cases. The `LIMIT`/`OFFSET` pattern also works identically across MySQL and PostgreSQL.

### D8: SQL formatting in the frontend, not the Rust backend

**Decision**: `sql-formatter` (npm) is called directly in the Svelte frontend for the editor Format button. The `query_format` Tauri command exists for server-side cases (e.g., during SQL file import).

**Rationale**: Avoids a round-trip for a pure, synchronous, deterministic transformation. `sql-formatter` is an npm package, so calling it in the frontend is natural.

### D9: DashMap for the connection pool registry

**Decision**: The active connection pool map uses `dashmap::DashMap` rather than `tokio::sync::RwLock<HashMap>`.

**Rationale**: Multiple Tauri command handlers run concurrently on the async Tokio runtime. `DashMap` provides fine-grained per-shard locking without a single read/write lock that would serialise all pool lookups. Pool lookup (the hot path for every query) is a shared read operation.

---

## 17. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `russh` SSH tunnel stability under long-running connections | Medium | High | Implement keepalive pings; auto-restart tunnel on error; surface clear disconnection UI |
| `keyring` crate platform inconsistencies (especially Linux with multiple keychain backends) | High | Medium | Test against both libsecret (GNOME Keyring) and KWallet; provide fallback encrypted file store with passphrase for Linux |
| elkjs layout performance on large schemas (100+ tables) | Medium | Medium | Run elkjs in a Web Worker; implement incremental layout (only lay out visible subgraph); add "compact" layout option |
| sqlx `AnyPool` losing type info for some column types | Low | Medium | Fall back to `TEXT` representation for unknown types; allow user to override column type in results display |
| Tauri v2 API surface changes before stable | Low | High | Pin to a specific Tauri v2 release; review changelog before upgrading |
| CSS variables theme approach causing FOUC (flash of unstyled content) | Medium | Low | Apply `data-theme` attribute from `localStorage` in a `<script>` in `app.html` before Svelte mounts |
| CodeMirror schema autocomplete becoming stale after DDL changes | Medium | Low | Provide "Refresh Schema" button in schema tree context menu; auto-refresh on reconnect |

---

## 18. Dependency Reference

### Cargo.toml (src-tauri)

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-updater = "2"
tauri-plugin-fs = "2"
tauri-plugin-dialog = "2"
tauri-plugin-shell = "2"
sqlx = { version = "0.8", features = [
    "mysql", "postgres", "sqlite",
    "runtime-tokio-rustls",
    "uuid", "chrono", "json"
] }
keyring = "2"
russh = "0.44"
russh-keys = "0.44"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4", "serde"] }
anyhow = "1"
thiserror = "1"
csv = "1"
dashmap = "6"

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

### package.json (key entries)

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-fs": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "@tauri-apps/plugin-updater": "^2",
    "@tanstack/svelte-table": "^8",
    "bits-ui": "latest",
    "@codemirror/autocomplete": "^6",
    "@codemirror/lang-sql": "^6",
    "@codemirror/state": "^6",
    "@codemirror/theme-one-dark": "^6",
    "@codemirror/view": "^6",
    "codemirror": "^6",
    "d3-drag": "^3",
    "d3-zoom": "^3",
    "elkjs": "^0.9",
    "fuse.js": "^7",
    "sql-formatter": "^15",
    "svelte": "^5",
    "web-worker": "^1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "vite": "^6",
    "typescript": "^5"
  }
}
```

---

### Critical Files for Implementation

These are the highest-leverage starting points and most architecturally significant pieces:

- `src-tauri/src/connections/pool_manager.rs` — central connection lifecycle; everything else in the backend depends on it
- `src-tauri/src/db/migrations/0001_init.sql` — must be finalised before any other backend code; the schema shapes all data models
- `src/lib/components/layout/SplitPanel.svelte` — load-bearing for the entire UI; getting its state model right early prevents expensive rewrites
- `src/lib/stores/panels.svelte.ts` — shared by layout, sidebar, keyboard shortcuts, and command palette; data model must be designed carefully
- `src/styles/variables.css` — the complete token system must be designed upfront; adding variables later is easy, renaming them cascades through every component

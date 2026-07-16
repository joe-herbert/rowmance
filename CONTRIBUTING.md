# Contributing to Rowmance

Thanks for your interest in contributing. This document covers the conventions we use to keep the codebase consistent and the review process smooth.

---

## Branch Conventions

| Prefix | Use for |
|---|---|
| `feature/` | New functionality |
| `fix/` | Bug fixes |
| `chore/` | Maintenance, dependency updates, tooling |

Examples: `feature/csv-export`, `fix/connection-retry`, `chore/update-tauri`.

Always branch from `main`. Keep branches short-lived — aim to merge within a week.

---

## PR Process

1. **Open a draft PR early** if you want feedback on direction before the implementation is complete.
2. **Fill in the PR description** — include what changed, why, and how to test it. PRs without descriptions will not be merged.
3. **All CI checks must pass** before requesting review:
   - `make check` (frontend: svelte-check, tsc, eslint, prettier)
   - `make rust-check` (rustfmt, clippy, cargo test)
4. Request at least one reviewer. Self-merge is not allowed on `main`.
5. Squash or merge — no preference, but keep the history readable.

---

## Testing Guide

### Frontend tests

```bash
make test           # run once
make test-watch     # interactive watch mode
make test-coverage  # with coverage report
```

Tests live alongside the code they test (e.g. `foo.svelte.ts` has `foo.test.ts`). Smoke tests live in `src/tests/`.

All tests mock Tauri's `invoke()` via the setup in `src/tests/setup.ts` — no real backend is required to run the frontend test suite.

### Rust tests

```bash
make rust-test
```

Use `SQLX_OFFLINE=true` when running cargo commands locally if you do not have a live database available (the Makefile handles this automatically).

---

## Code Style

- **Prettier** for all frontend files — run `make format` to auto-fix.
- **rustfmt** for Rust — run `make rust-fmt` to auto-fix.
- **Clippy** must pass with no warnings (`-D warnings`).
- **Comments**: only write comments when the *why* is non-obvious. Code that explains itself needs no comment.
- **No `any` types** in TypeScript — use proper types or generics.
- **Svelte 5 runes syntax**: use `$state`, `$derived`, `$effect` — not the legacy store API.
- **CSS variables**: use tokens from `src/styles/variables.css` (e.g. `var(--color-bg-primary)`) — no hard-coded colours.

---

## Adding a New Database Engine

The architecture is designed so that adding an engine requires exactly one new file and four small edits to existing files. No command handlers, no frontend components, and no SQL generation utilities need to change.

### How it works

Every engine implements the `DatabaseEngine` trait (`src-tauri/src/connections/engine.rs`). Command handlers call trait methods and have no knowledge of which engine is behind a connection.

The frontend receives a `DialectInfo` struct alongside every connection profile — a data bag that encodes all engine-specific UI and SQL-generation decisions as plain values. The frontend never branches on engine name strings; it reads capability fields from `DialectInfo` instead.

All engine registration lives in `src-tauri/src/connections/engines/mod.rs`. That is the single file to update when wiring in a new engine.

### Step 1 — `src-tauri/Cargo.toml`

Add whatever crate provides your database driver (e.g. `sqlx` with a feature flag, or a standalone crate like `tiberius`).

### Step 2 — `src-tauri/src/connections/engines/myengine.rs`

This is the only file you write from scratch. It must export two things:

**`dialect_info(db_type: &str) -> Option<DialectInfo>`** — returns `Some(DialectInfo { … })` for every `db_type` string your engine handles (e.g. both `"myengine"` and `"myengine_alt"`), and `None` for anything else.

Every `DialectInfo` field must have a value — the compiler will reject a missing field. Use an existing engine as a reference; the field names map directly to frontend capability checks. Key fields to think through:

| Field | Controls |
|---|---|
| `identifier_open` / `identifier_close` / `identifier_escape` | Identifier quoting in generated SQL |
| `uses_schema` | Whether the connection tree shows a schema level |
| `is_file_based` | Shows file-path input instead of host/port in the connection form |
| `explain_format` | How EXPLAIN output is parsed: `"mysql_json"`, `"postgres_json"`, `"sqlite_queryplan"`, or `"sqlserver_xml"` |
| `supports_user_management` | Whether the Users tab is shown |
| `default_port` / `url_schemes` / `url_template` | Connection URL parsing and the default port pre-fill |
| `editor_dialect` | Syntax highlighting mode: `"mysql"`, `"postgresql"`, or `"sql"` |

**`create_pool(…) -> Result<Box<dyn PoolAdapter>, RowmanceError>`** — opens and returns a connection pool wrapped in a type that implements `PoolAdapter`:

```rust
pub struct MyEnginePoolAdapter { pub pool: my_driver::Pool }

impl fmt::Debug for MyEnginePoolAdapter { … }

#[async_trait]
impl PoolAdapter for MyEnginePoolAdapter {
    async fn disconnect(&self) { self.pool.close().await; }
    async fn ping(&self) -> bool { /* lightweight probe */ }
    fn get_engine(&self) -> Arc<dyn DatabaseEngine> {
        Arc::new(MyEngine { pool: self.pool.clone() })
    }
}

pub async fn create_pool(host: &str, port: u16, …) -> Result<Box<dyn PoolAdapter>, RowmanceError> {
    let pool = /* build pool */;
    Ok(Box::new(MyEnginePoolAdapter { pool }))
}
```

**`DatabaseEngine` impl** — a struct wrapping the pool that implements every required method on the `DatabaseEngine` trait:

```rust
pub struct MyEngine { pub pool: my_driver::Pool }

#[async_trait]
impl DatabaseEngine for MyEngine {
    fn quote(&self, ident: &str) -> String { /* engine-specific quoting */ }
    fn placeholder(&self, n: usize) -> String { /* "?" vs "$1" vs "@P1" */ }
    fn build_pagination(&self, sql: &str, limit: u64, offset: u64) -> String { /* … */ }
    fn parse_returns_rows(&self, sql: &str) -> bool { /* … */ }

    async fn list_databases(&self) -> Result<Vec<String>, RowmanceError> { /* … */ }
    async fn list_tables(&self, database: &str) -> Result<Vec<TableInfo>, RowmanceError> { /* … */ }
    // … all remaining required methods
    async fn import_csv(&self, …) -> Result<u64, RowmanceError> { /* … */ }
}
```

User management methods (`list_users`, `create_user`, `drop_user`, etc.) have default implementations that return an "unsupported" error — only override them if your engine supports user management.

`begin_transaction` and `begin_session` must return a `Box<dyn EngineTransaction>`. Define a companion struct (e.g. `MyEngineTransaction`) in the same file that wraps a checked-out connection and implements `EngineTransaction`.

### Step 3 — `src-tauri/src/connections/engines/mod.rs`

Four additions to this file:

```rust
// 1. Declare the module at the top of the file
pub mod myengine;

// 2. Add to dialect_for_db_type() — the .or_else() chain
pub fn dialect_for_db_type(db_type: &str) -> DialectInfo {
    mysql::dialect_info(db_type)
        .or_else(|| postgres::dialect_info(db_type))
        .or_else(|| sqlite::dialect_info(db_type))
        .or_else(|| sqlserver::dialect_info(db_type))
        .or_else(|| myengine::dialect_info(db_type))  // ← add this
        .unwrap_or_else(|| default_dialect(db_type))
}

// 3. Add to all_known_dialects() — populates the engine dropdown in the UI
pub fn all_known_dialects() -> Vec<(String, DialectInfo)> {
    vec![
        // … existing entries …
        ("myengine".to_owned(), myengine::dialect_info("myengine").unwrap()),
    ]
}

// 4. Add to connect_for_db_type() — opens the pool when a connection is created
match db_type {
    // … existing arms …
    "myengine" => myengine::create_pool(host, port, database, username, password, pool_max, ssl_enabled, ssl_ca_path, read_only).await,
    other => Err(RowmanceError::ConnectionNotFound(format!("Unknown db_type: {other}")))
}
```

That's everything. The pool manager, command handlers, and all frontend code pick up the new engine automatically.

### Special case: new EXPLAIN format

If your engine's EXPLAIN output is not structurally equivalent to any of the four existing formats (`mysql_json`, `postgres_json`, `sqlite_queryplan`, `sqlserver_xml`), you will need to add a parser and a branch in `src/lib/components/explain/ExplainCanvas.svelte`. Set `explain_format` in your `DialectInfo` to match the new format string you add there.

### Verify

```bash
make check        # svelte-check, tsc, eslint, prettier
make rust-check   # rustfmt, clippy, cargo test
```

Manually test at minimum: connect → browse tables → run a query → edit a cell → save.

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

## Releasing

`scripts/bump-version.sh` updates the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` together (plus the matching entry in `src-tauri/Cargo.lock`, which `cargo build`/`check` would otherwise rewrite on the next run), so they never drift out of sync.

```bash
./scripts/bump-version.sh                  # bump patch: 2.3.4 -> 2.3.5
./scripts/bump-version.sh 2.4.0            # set an explicit version
./scripts/bump-version.sh 2.4.0 --commit   # also commit as "Bump to v2.4.0"
./scripts/bump-version.sh 2.4.0 --tag      # commit and tag as "v2.4.0" (implies --commit)
```

With no version argument, it increments the last dot-separated field of the current version (read from `package.json`). `--commit` and `--tag` are both opt-in — with neither, the script only edits the three files and leaves staging/committing to you.

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

## Test databases

Rowmance ships a `docker-compose.yml` that spins up a local instance of every database engine the client supports, each pre-seeded with the same `customers` / `orders` schema. Use these for manual testing and for anything that needs a real database rather than a mock.

### Prerequisites

- Docker and Docker Compose
- `make` (already required by the other targets in this Makefile)

### Starting and stopping

```bash
make test-db-up      # start all test databases in the background
make test-db-down     # stop and remove the containers (data persists in volumes)
```

Under the hood these just wrap `docker compose -f docker-compose.yml up -d` / `down`.

To wipe all data and start from a completely clean state (needed after changing anything in `init-scripts/`, since most engines only run their init scripts on a fresh volume):

```bash
make test-db-down
docker compose down -v
rm -rf ./data/sqlite
make test-db-up
```

### Connecting

All engines use the same credentials pattern (`test1234` / `TestPass123` depending on the engine's password rules) and the same database/schema name, `testdb`, so you can swap connection strings without re-learning setup each time.

| Engine     | Connection URL                                                  |
|------------|-------------------------------------------------------------------|
| Postgres   | `postgresql://postgres:test1234@localhost:5432/testdb`            |
| SQL Server | `sqlserver://sa:TestPass123%23@localhost:1433/testdb`             |
| MySQL      | `mysql://root:TestPass123@localhost:3306/testdb`                  |
| MariaDB    | `mysql://root:TestPass123@localhost:3307/testdb`                  |
| Oracle     | `oracle://appuser:TestPass123@localhost:1521/testdb`              |
| SQLite     | `sqlite:///./data/sqlite/testdb.db` (path relative to repo root)  |

Notes:

- **SQL Server**: the `#` in the password must be percent-encoded (`%23`) in URL form. `dbo` is the default *schema* inside `testdb`, not a separate database — if your client lists it as a sibling of `testdb` rather than nested underneath, that's a display bug in the client, not the database.
- **Oracle**: connect as `appuser`, not `system`. The seed script creates the tables under the `appuser` schema so they're visible without schema-qualifying every query.
- **MariaDB**: runs on host port `3307` (not the default `3306`) so it can run alongside MySQL without a port clash.
- **SQLite**: there's no server to connect to — your client just opens the `.db` file directly from disk. If you need foreign keys enforced, your client must run `PRAGMA foreign_keys = ON` per connection; SQLite doesn't persist this setting.

### Seed schema

Every engine seeds two tables — `customers` (parent) and `orders` (child, FK'd on `customer_id` with `ON DELETE CASCADE`) — deliberately covering a wide spread of column types so client features can be exercised against all of them: auto-increment/identity primary keys, UUID/GUID, short and long text, boolean, decimal/numeric, float, date, timestamp, JSON, binary/BLOB, and a `CHECK`-constrained status column. Some rows include `NULL`s to test nullable-column handling.

Seed SQL lives in `init-scripts/<engine>/01-seed.sql`. If you need to add columns or test cases, edit the relevant file per engine (the SQL dialects differ enough that there's a separate script per engine rather than one shared file) — then do a full `docker compose down -v` reset for the change to take effect, since most engines only run these scripts once, on first initialization of an empty volume.

### Gotchas

- **First-boot-only init**: `POSTGRES_DB`, `MYSQL_DATABASE`, `MARIADB_DATABASE`, and Oracle's startup scripts only run when a volume is created empty. Editing a seed script and restarting the container without also dropping its volume will silently do nothing.
- **`mssql-init` is a one-shot container**: it creates `testdb` and runs SQL Server's seed script, then exits successfully. It won't appear in `docker compose ps` once done — that's expected, not a failure. It reruns its `CREATE DATABASE IF NOT EXISTS` logic on every `docker compose up`, but the seed *data* inserts are not idempotent, so rerunning it against an already-seeded `testdb` will fail on unique constraints. That's fine for local dev; just don't expect repeated `up`s to reset SQL Server's data — use the full volume reset above instead.
- **Startup time**: Oracle is the slowest engine to become ready (up to ~1-2 minutes on first boot). If you're scripting against it (e.g. in CI), wait/retry rather than assuming a fast failure means something's broken.

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

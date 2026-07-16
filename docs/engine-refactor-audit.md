# Engine Refactor Audit

Audit of all locations with hardcoded engine-specific logic that would require manual updates when adding a new database engine. The goal of the refactor is that adding a new engine file should make it work everywhere — no other file should need to be touched.

---

## Rust Backend

### `src-tauri/src/connections/types.rs`
- **Lines 166–378**: `DialectInfo::for_db_type()` — match on db_type string with a case per engine
- **Lines 372–380**: `DialectInfo::all_known()` — hardcoded list of all engine names

### `src-tauri/src/connections/pool_manager.rs`
- **Lines 43–48**: `RemotePool` enum — a variant per engine; every match on it needs updating
- **Lines 90–275**: `connect()` — match on db_type string creating engine-specific pool types
- **Lines 325–332**: `disconnect()` — match on all `RemotePool` variants
- **Lines 355–359**: `ping()` — match on all `RemotePool` variants
- **Lines 407–418**: `get_engine()` — match creating engine trait objects per variant

### `src-tauri/src/commands/query.rs`
- **Lines 92–99**: `quote_mysql()` and `quote_postgres()` — per-engine quoting functions, not delegated to the engine trait

---

## TypeScript / Svelte Frontend

### `src/lib/types/index.ts`
- **Line 6**: `DbType` union type — lists all 5 engines explicitly

### `src/lib/components/connections/ConnectionForm.svelte`
- **Lines 413–417**: Hardcoded array of engine options for the connection type dropdown

### `src/lib/components/palette/GlobalSearch.svelte`
- **Lines 155–161**: `dbTypeLabels` — `Record<DbType, string>` mapping engine names to display labels

### `src/lib/components/editor/QueryEditor.svelte`
- **Lines 637–641**: `DB_TYPE_DIALECT` — hardcoded map of db_type → syntax highlighter dialect (SQLite and SQL Server are missing, silently fall back to `'sql'`)

### `src/lib/components/explain/ExplainCanvas.svelte`
- **Line 214**: `if (dialect === 'sqlserver')` — SQL Server EXPLAIN is XML, checked by name
- **Line 230**: `dialect === 'postgres' ? parsePostgres() : parseMysql()` — assumes non-postgres is MySQL JSON format; any new engine silently uses the wrong parser

### `src/lib/components/schema/TableStructure.svelte`
- **Lines 86–89**: `isMysql`, `isPostgres`, `isSqlite`, `isSqlServer` derived from dialect capability flags

### `src/lib/components/users/UserManager.svelte`
- **Lines 29–31**: `isMysql`, `isPostgres`, `isSqlite` derived from dialect capability flags

### `src/lib/components/table/TableBrowser.svelte`
- **Lines 653–658**: Fallback quoting uses MySQL backtick style for unknown engines

### `src/lib/utils/dialect.ts`
- **Lines 6–36**: Hardcoded default `DialectInfo` with MySQL-specific defaults as the fallback

---

## Priority Issues

These two will silently produce wrong results (not just missing support) for any new engine:

1. **`ExplainCanvas.svelte:230`** — the `else` branch assumes MySQL JSON EXPLAIN format; a new engine gets the wrong parser with no error
2. **`query.rs:92–99`** — quoting logic is not on the engine trait, so a new engine cannot define its own quoting strategy

The remaining issues are enumeration problems where the new engine simply won't appear or will be skipped — easier to catch at runtime or in testing.

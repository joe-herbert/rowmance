/// SQLite-specific schema introspection queries.
use sqlx::SqlitePool;

use crate::connections::types::{
    CheckConstraintInfo, ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo, TriggerInfo, ViewInfo,
};
use crate::error::RowmanceError;

/// SQLite has a single implicit schema; we return ["main"] so the connection
/// tree has a database node to expand into tables.
pub async fn list_databases(_pool: &SqlitePool) -> Result<Vec<String>, RowmanceError> {
    Ok(vec!["main".to_owned()])
}

/// List all user tables and views in the SQLite database.
pub async fn list_tables(
    pool: &SqlitePool,
    _database: &str,
) -> Result<Vec<TableInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        table_type: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT name, type AS table_type FROM sqlite_master \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    let tables: Vec<TableInfo> = rows
        .into_iter()
        .map(|r| TableInfo {
            name: r.name,
            table_type: if r.table_type == "view" {
                "view".to_owned()
            } else {
                "table".to_owned()
            },
            row_count: None,
        })
        .collect();

    Ok(tables)
}

/// Count all rows in a table. Used by the background count task.
pub async fn count_table(pool: &SqlitePool, table: &str) -> Result<i64, RowmanceError> {
    let tbl_esc = table.replace('"', "\"\"");
    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{}\"", tbl_esc))
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// List all columns for a given table using PRAGMA table_info.
pub async fn list_columns(
    pool: &SqlitePool,
    _database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        #[sqlx(rename = "type")]
        data_type: String,
        notnull: i64,
        dflt_value: Option<String>,
        pk: i64,
    }

    let fk_cols = foreign_key_columns(pool, table).await;

    let sql = format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\""));
    let rows = sqlx::query_as::<_, Row>(&sql).fetch_all(pool).await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let is_fk = fk_cols.contains(&r.name);
            ColumnInfo {
                name: r.name,
                data_type: r.data_type,
                nullable: r.notnull == 0,
                default_value: r.dflt_value,
                is_primary_key: r.pk != 0,
                is_auto_increment: false,
                is_foreign_key: is_fk,
                comment: None,
            }
        })
        .collect())
}

/// List all columns for every user table and view in one Rust call.
/// SQLite has no cross-table column query, so we fetch table names first
/// and then issue PRAGMA calls — but this still uses only one IPC round-trip.
pub async fn list_all_columns(
    pool: &SqlitePool,
    _database: &str,
) -> Result<Vec<(String, ColumnInfo)>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct TableRow {
        name: String,
    }

    let tables = sqlx::query_as::<_, TableRow>(
        "SELECT name FROM sqlite_master \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    let mut result: Vec<(String, ColumnInfo)> = Vec::new();

    for t in tables {
        let fk_cols = foreign_key_columns(pool, &t.name).await;

        #[derive(sqlx::FromRow)]
        struct ColRow {
            name: String,
            #[sqlx(rename = "type")]
            data_type: String,
            notnull: i64,
            dflt_value: Option<String>,
            pk: i64,
        }

        let sql = format!("PRAGMA table_info(\"{}\")", t.name.replace('"', "\"\""));
        let cols = sqlx::query_as::<_, ColRow>(&sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        for col in cols {
            let is_fk = fk_cols.contains(&col.name);
            result.push((
                t.name.clone(),
                ColumnInfo {
                    name: col.name,
                    data_type: col.data_type,
                    nullable: col.notnull == 0,
                    default_value: col.dflt_value,
                    is_primary_key: col.pk != 0,
                    is_auto_increment: false,
                    is_foreign_key: is_fk,
                    comment: None,
                },
            ));
        }
    }

    Ok(result)
}

async fn foreign_key_columns(pool: &SqlitePool, table: &str) -> std::collections::HashSet<String> {
    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(rename = "from")]
        from_col: String,
    }

    let sql = format!(
        "PRAGMA foreign_key_list(\"{}\")",
        table.replace('"', "\"\"")
    );
    sqlx::query_as::<_, Row>(&sql)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.from_col)
        .collect()
}

/// List all indexes for a given table.
pub async fn list_indexes(
    pool: &SqlitePool,
    _database: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct IndexListRow {
        name: String,
        unique: i64,
    }

    #[derive(sqlx::FromRow)]
    struct IndexInfoRow {
        name: Option<String>,
    }

    let list_sql = format!("PRAGMA index_list(\"{}\")", table.replace('"', "\"\""));
    let indexes = sqlx::query_as::<_, IndexListRow>(&list_sql)
        .fetch_all(pool)
        .await?;

    let mut result = Vec::new();
    for idx in indexes {
        let info_sql = format!("PRAGMA index_info(\"{}\")", idx.name.replace('"', "\"\""));
        let cols = sqlx::query_as::<_, IndexInfoRow>(&info_sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        let columns: Vec<String> = cols.into_iter().filter_map(|r| r.name).collect();

        result.push(IndexInfo {
            name: idx.name,
            columns,
            unique: idx.unique != 0,
            index_type: "BTREE".to_owned(),
        });
    }

    Ok(result)
}

/// List all foreign keys for a given table.
pub async fn list_foreign_keys(
    pool: &SqlitePool,
    _database: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        #[sqlx(rename = "table")]
        ref_table: String,
        #[sqlx(rename = "from")]
        from_col: String,
        #[sqlx(rename = "to")]
        to_col: Option<String>,
        on_delete: String,
        on_update: String,
    }

    let sql = format!(
        "PRAGMA foreign_key_list(\"{}\")",
        table.replace('"', "\"\"")
    );
    let rows = sqlx::query_as::<_, Row>(&sql).fetch_all(pool).await?;

    let mut map: std::collections::BTreeMap<i64, ForeignKeyInfo> =
        std::collections::BTreeMap::new();
    for r in rows {
        let entry = map.entry(r.id).or_insert_with(|| ForeignKeyInfo {
            constraint_name: format!("fk_{table}_{}", r.id),
            columns: vec![],
            referenced_table: r.ref_table.clone(),
            referenced_columns: vec![],
            on_delete: r.on_delete.clone(),
            on_update: r.on_update.clone(),
        });
        entry.columns.push(r.from_col);
        if let Some(to) = r.to_col {
            entry.referenced_columns.push(to);
        }
    }

    Ok(map.into_values().collect())
}

/// List all views in the SQLite database. The `definition` is the full
/// `CREATE VIEW ...` statement from sqlite_master, since SQLite does not
/// cleanly separate the view name from its underlying SELECT.
pub async fn list_views(pool: &SqlitePool, _database: &str) -> Result<Vec<ViewInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        sql: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT name, sql FROM sqlite_master WHERE type = 'view' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ViewInfo {
            name: r.name,
            definition: r.sql.unwrap_or_default(),
        })
        .collect())
}

/// List all indexes for every table in one Rust call. SQLite has no
/// cross-table index catalog query, so we fetch table names first and loop,
/// reusing the same PRAGMA calls as the per-table `list_indexes`.
pub async fn list_all_indexes(
    pool: &SqlitePool,
    database: &str,
) -> Result<Vec<(String, IndexInfo)>, RowmanceError> {
    let tables = list_tables(pool, database).await?;

    let mut result: Vec<(String, IndexInfo)> = Vec::new();
    for t in tables {
        let indexes = list_indexes(pool, database, &t.name).await.unwrap_or_default();
        for idx in indexes {
            result.push((t.name.clone(), idx));
        }
    }

    Ok(result)
}

/// List all foreign keys for every table in one Rust call, looping over
/// `PRAGMA foreign_key_list` per table (same approach as `list_all_indexes`).
pub async fn list_all_foreign_keys(
    pool: &SqlitePool,
    database: &str,
) -> Result<Vec<(String, ForeignKeyInfo)>, RowmanceError> {
    let tables = list_tables(pool, database).await?;

    let mut result: Vec<(String, ForeignKeyInfo)> = Vec::new();
    for t in tables {
        let fks = list_foreign_keys(pool, database, &t.name)
            .await
            .unwrap_or_default();
        for fk in fks {
            result.push((t.name.clone(), fk));
        }
    }

    Ok(result)
}

/// Return the CREATE TABLE / CREATE VIEW DDL from sqlite_master.
pub async fn get_ddl(pool: &SqlitePool, table: &str) -> Result<String, RowmanceError> {
    let ddl: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE name = ? AND type IN ('table', 'view')",
    )
    .bind(table)
    .fetch_optional(pool)
    .await?;

    Ok(ddl.unwrap_or_default())
}

/// List CHECK constraints. Pass `table: Some(name)` to scope to a single
/// table, or `None` to fetch every table's check constraints in the database.
///
/// SQLite has no catalog for check constraints at all — they must be parsed
/// out of the table's own `CREATE TABLE` DDL text, which is available from
/// `sqlite_master.sql`. See `parse_check_constraints_from_ddl` for the
/// (best-effort) parsing logic.
pub async fn list_check_constraints(
    pool: &SqlitePool,
    _database: &str,
    table: Option<&str>,
) -> Result<Vec<CheckConstraintInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        sql: Option<String>,
    }

    let rows: Vec<Row> = if let Some(table) = table {
        sqlx::query_as::<_, Row>(
            "SELECT name, sql FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(pool)
        .await?
    };

    let mut result = Vec::new();
    for r in rows {
        if let Some(ddl) = r.sql {
            result.extend(parse_check_constraints_from_ddl(&r.name, &ddl));
        }
    }
    Ok(result)
}

/// Returns true if `b` can be part of a bare SQL identifier (used to check
/// word boundaries around the `CHECK`/`CONSTRAINT` keywords below).
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Best-effort extraction of `CHECK (...)` constraint clauses from a SQLite
/// `CREATE TABLE` statement's DDL text (as returned by `sqlite_master.sql`).
///
/// This is NOT a full SQL parser. It scans the DDL char-by-char for
/// occurrences of the `CHECK` keyword (case-insensitive, word-bounded), then
/// walks forward from the following `(` tracking paren depth (and skipping
/// over string literals) to find the matching closing paren — this correctly
/// handles expressions with nested parens such as
/// `CHECK (price > (cost * 1.1))`, which a naive regex would mishandle.
/// It also looks backward from each `CHECK` for an immediately preceding
/// `CONSTRAINT <name>` clause to use as the constraint's name; if none is
/// found, a synthetic name `check_<table>_<index>` is generated (mirroring
/// the synthesized `fk_<table>_<id>` naming already used for unnamed SQLite
/// foreign keys in `list_foreign_keys`).
///
/// Exotic formatting — `CHECK` appearing inside comments or string literals
/// disguised as identifiers, unusual whitespace, etc. — may confuse this
/// parser. It is meant to catch the common cases produced by normal
/// `CREATE TABLE` statements, not to be a fully correct SQL tokenizer.
fn parse_check_constraints_from_ddl(table: &str, ddl: &str) -> Vec<CheckConstraintInfo> {
    let bytes = ddl.as_bytes();
    let len = bytes.len();
    let lower = ddl.to_ascii_lowercase();

    let mut results = Vec::new();
    let mut search_start = 0usize;

    while let Some(rel_pos) = lower[search_start..].find("check") {
        let pos = search_start + rel_pos;
        let after_idx = pos + "check".len();

        let before_ok = pos == 0 || !is_ident_byte(bytes[pos - 1]);
        let after_ok = after_idx >= len || !is_ident_byte(bytes[after_idx]);
        if !before_ok || !after_ok {
            search_start = pos + 1;
            continue;
        }

        // Skip whitespace after CHECK; the next non-whitespace char must be
        // '(' for this to be a real CHECK(...) clause.
        let mut i = after_idx;
        while i < len && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= len || bytes[i] != b'(' {
            search_start = pos + 1;
            continue;
        }

        let open_pos = i;
        match find_matching_close_paren(bytes, open_pos) {
            Some(close_pos) => {
                let expression = ddl[open_pos..=close_pos].to_string();
                let name = find_preceding_constraint_name(ddl, pos)
                    .unwrap_or_else(|| format!("check_{table}_{}", results.len()));
                results.push(CheckConstraintInfo {
                    constraint_name: name,
                    table_name: table.to_string(),
                    expression,
                });
                search_start = close_pos + 1;
            }
            None => {
                // Unbalanced parens (shouldn't happen for valid DDL) — bail
                // on this occurrence and keep scanning.
                search_start = pos + 1;
            }
        }
    }

    results
}

/// Starting at `open_pos` (the byte index of an opening `(`), scan forward
/// tracking paren depth to find the index of the matching closing `)`.
/// Skips over single- and double-quoted string literals (including doubled
/// quote escapes, e.g. `'it''s'`) so parens inside string content don't
/// affect the depth count.
fn find_matching_close_paren(bytes: &[u8], open_pos: usize) -> Option<usize> {
    let len = bytes.len();
    let mut depth = 0i32;
    let mut in_string: Option<u8> = None;
    let mut j = open_pos;
    while j < len {
        let c = bytes[j];
        if let Some(quote) = in_string {
            if c == quote {
                if j + 1 < len && bytes[j + 1] == quote {
                    j += 1; // doubled-quote escape, stays inside the string
                } else {
                    in_string = None;
                }
            }
        } else {
            match c {
                b'\'' | b'"' => in_string = Some(c),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                _ => {}
            }
        }
        j += 1;
    }
    None
}

/// Look backward from `check_pos` (the byte offset of the `CHECK` keyword)
/// for an immediately preceding `CONSTRAINT <name>` clause — only whitespace
/// may separate them. `<name>` may be a bare identifier or quoted with
/// `"..."`, `` `...` ``, or `[...]`. Returns the (unquoted) name if found.
fn find_preceding_constraint_name(ddl: &str, check_pos: usize) -> Option<String> {
    let bytes = ddl.as_bytes();

    let mut i = check_pos;
    while i > 0 && (bytes[i - 1] as char).is_whitespace() {
        i -= 1;
    }
    let name_end = i;
    if i == 0 {
        return None;
    }

    let name_start = if matches!(bytes[i - 1], b'"' | b'`' | b']') {
        let close = bytes[i - 1];
        if close == b']' {
            // Search backward for the matching '[', tracking nested brackets.
            let mut depth = 0i32;
            let mut m = i - 1;
            loop {
                match bytes[m] {
                    b']' => depth += 1,
                    b'[' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                if m == 0 {
                    return None;
                }
                m -= 1;
            }
            m
        } else {
            // Search backward for the matching quote character.
            if i < 2 {
                return None;
            }
            let mut m = i - 2;
            loop {
                if bytes[m] == close {
                    break;
                }
                if m == 0 {
                    return None;
                }
                m -= 1;
            }
            m
        }
    } else if is_ident_byte(bytes[i - 1]) {
        let mut k = i - 1;
        while k > 0 && is_ident_byte(bytes[k - 1]) {
            k -= 1;
        }
        k
    } else {
        return None;
    };

    let raw_name = &ddl[name_start..name_end];

    // Now expect whitespace-separated "CONSTRAINT" immediately before the name.
    let mut j = name_start;
    while j > 0 && (bytes[j - 1] as char).is_whitespace() {
        j -= 1;
    }
    let kw_end = j;
    let kw_len = "constraint".len();
    if kw_end < kw_len {
        return None;
    }
    let kw_start = kw_end - kw_len;
    let candidate = &ddl[kw_start..kw_end];
    if !candidate.eq_ignore_ascii_case("constraint") {
        return None;
    }
    let boundary_ok = kw_start == 0 || !is_ident_byte(bytes[kw_start - 1]);
    if !boundary_ok {
        return None;
    }

    let cleaned = raw_name.trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']');
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

/// List triggers. Pass `table: Some(name)` to scope to a single table, or
/// `None` to fetch every table's triggers in the database.
///
/// Unlike check constraints, SQLite's `sqlite_master` has a dedicated
/// `type = 'trigger'` row per trigger, so no DDL-scanning is needed to
/// *enumerate* them. But the catalog gives no structured `timing`/`event`
/// columns — those must still be parsed out of the trigger's own `sql` text
/// (the full `CREATE TRIGGER` statement), which is used as-is for
/// `definition`. See `parse_trigger_timing_and_event` for the (best-effort)
/// parsing logic.
pub async fn list_triggers(
    pool: &SqlitePool,
    _database: &str,
    table: Option<&str>,
) -> Result<Vec<TriggerInfo>, RowmanceError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        tbl_name: String,
        sql: Option<String>,
    }

    let rows: Vec<Row> = if let Some(table) = table {
        sqlx::query_as::<_, Row>(
            "SELECT name, tbl_name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = ? ORDER BY name",
        )
        .bind(table)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT name, tbl_name, sql FROM sqlite_master WHERE type = 'trigger' ORDER BY tbl_name, name",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|r| {
            let ddl = r.sql.unwrap_or_default();
            let (timing, event) = parse_trigger_timing_and_event(&ddl);
            TriggerInfo {
                name: r.name,
                table_name: r.tbl_name,
                timing,
                event,
                definition: ddl,
            }
        })
        .collect())
}

/// Best-effort extraction of a SQLite trigger's timing (`BEFORE`/`AFTER`/
/// `INSTEAD OF`) and event (`INSERT`/`UPDATE`/`DELETE`) from its own
/// `CREATE TRIGGER` DDL text (as returned by `sqlite_master.sql`).
///
/// This is NOT a full SQL parser. SQLite's `CREATE TRIGGER` grammar is:
/// `CREATE [TEMP|TEMPORARY] TRIGGER [IF NOT EXISTS] [schema.]name
///  [BEFORE|AFTER|INSTEAD OF] (DELETE|INSERT|UPDATE [OF column-list]) ON table-name ...`
/// so this tokenizes the DDL (see `tokenize_sql_keywords`), locates the
/// `TRIGGER` keyword, skips past an optional `IF NOT EXISTS` and the
/// trigger's (possibly schema-qualified, possibly quoted) name, and then
/// reads the following one or two tokens positionally for timing and event.
/// Per SQLite's documentation, if none of `BEFORE`/`AFTER`/`INSTEAD OF` is
/// specified the trigger defaults to firing `BEFORE` the event, so that is
/// used as the fallback when no explicit timing keyword is found. Exotic
/// formatting or comments between keywords may confuse this parser — it is
/// meant to catch the common cases produced by normal `CREATE TRIGGER`
/// statements, not to be a fully correct SQL tokenizer.
fn parse_trigger_timing_and_event(ddl: &str) -> (String, String) {
    let tokens = tokenize_sql_keywords(ddl);
    let upper: Vec<String> = tokens.iter().map(|t| t.to_ascii_uppercase()).collect();

    let Some(trigger_idx) = upper.iter().position(|t| t == "TRIGGER") else {
        return ("BEFORE".to_string(), String::new());
    };

    let mut i = trigger_idx + 1;
    if upper.get(i).map(String::as_str) == Some("IF")
        && upper.get(i + 1).map(String::as_str) == Some("NOT")
        && upper.get(i + 2).map(String::as_str) == Some("EXISTS")
    {
        i += 3;
    }
    // Skip the (possibly schema-qualified) trigger name: name | name . name
    if i < upper.len() {
        i += 1;
        if upper.get(i).map(String::as_str) == Some(".") {
            i += 2;
        }
    }

    let mut timing = "BEFORE".to_string();
    match upper.get(i).map(String::as_str) {
        Some("BEFORE") => {
            timing = "BEFORE".to_string();
            i += 1;
        }
        Some("AFTER") => {
            timing = "AFTER".to_string();
            i += 1;
        }
        Some("INSTEAD") if upper.get(i + 1).map(String::as_str) == Some("OF") => {
            timing = "INSTEAD OF".to_string();
            i += 2;
        }
        _ => {}
    }

    let event = match upper.get(i).map(String::as_str) {
        Some("DELETE") => "DELETE".to_string(),
        Some("INSERT") => "INSERT".to_string(),
        Some("UPDATE") => "UPDATE".to_string(),
        _ => String::new(),
    };

    (timing, event)
}

/// Tokenizes SQL DDL text into a flat list of keyword/identifier/punctuation
/// tokens for lightweight positional keyword scanning. Quoted or bracketed
/// identifiers (`"..."`, `` `...` ``, `[...]`, `'...'`) are treated as a
/// single opaque token each (their contents are not tokenized further) so
/// that keywords appearing inside a quoted name don't confuse the scan. Not
/// a full SQL tokenizer — no numeric-literal or operator handling beyond
/// what `parse_trigger_timing_and_event` needs.
fn tokenize_sql_keywords(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < len {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match bytes[i] {
            b'"' | b'`' | b'\'' => {
                let quote = bytes[i];
                let start = i;
                i += 1;
                while i < len {
                    if bytes[i] == quote {
                        if i + 1 < len && bytes[i + 1] == quote {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                tokens.push(sql[start..i].to_string());
            }
            b'[' => {
                let start = i;
                i += 1;
                while i < len && bytes[i] != b']' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                tokens.push(sql[start..i].to_string());
            }
            b'.' | b'(' | b')' | b',' | b';' => {
                tokens.push((bytes[i] as char).to_string());
                i += 1;
            }
            _ if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < len {
                    let cc = bytes[i] as char;
                    if cc.is_alphanumeric() || cc == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                tokens.push(sql[start..i].to_string());
            }
            _ => {
                i += 1;
            }
        }
    }
    tokens
}

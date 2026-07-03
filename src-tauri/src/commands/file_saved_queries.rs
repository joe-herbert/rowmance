/// File-based saved queries: one .sql file per query, stored in a configurable
/// directory that can be committed to version control.
///
/// File format — the first block of `-- rowmance:<key>: <value>` lines is the
/// header; everything after (plus the blank separator line) is the SQL body.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::error::AppError;

// ── Constants ─────────────────────────────────────────────────────────────────

const HEADER_PREFIX: &str = "-- rowmance:";
const MAPPINGS_KEY: &str = "savedQueryConnectionMappings";
const DIR_SETTING_KEY: &str = "savedQueriesDirectory";
const ORDER_FILENAME: &str = "_order";

fn read_order_file(dir: &Path) -> Vec<String> {
    let path = dir.join(ORDER_FILENAME);
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

fn write_order_file(dir: &Path, items: &[String]) -> Result<(), AppError> {
    let path = dir.join(ORDER_FILENAME);
    if items.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    std::fs::write(&path, items.join("\n") + "\n")
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))
}

// ── Directory helpers ─────────────────────────────────────────────────────────

fn default_queries_dir() -> Result<PathBuf, AppError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| AppError::new("IO_ERROR", "Could not determine home directory"))?;
    Ok(home.join(".config").join("rowmance").join("saved_queries"))
}

async fn queries_dir(sqlite: &SqlitePool) -> Result<PathBuf, AppError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(DIR_SETTING_KEY)
            .fetch_optional(sqlite)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    if let Some((json,)) = row {
        if let Ok(Some(dir)) = serde_json::from_str::<Option<String>>(&json) {
            if !dir.is_empty() {
                return Ok(PathBuf::from(dir));
            }
        }
    }
    default_queries_dir()
}

// ── File format ───────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
struct FileHeader {
    connection_id: Option<String>,
    connection_fingerprint: Option<String>,
    database: Option<String>,
    position: Option<i64>,
}

fn parse_header(content: &str) -> (FileHeader, String) {
    let mut header = FileHeader::default();
    let mut lines = content.lines();
    let mut sql_lines: Vec<&str> = Vec::new();
    let mut past_header = false;

    for line in lines.by_ref() {
        if !past_header && line.starts_with(HEADER_PREFIX) {
            let rest = &line[HEADER_PREFIX.len()..];
            if let Some(colon) = rest.find(':') {
                let key = rest[..colon].trim();
                let val = rest[colon + 1..].trim().to_string();
                let opt = if val.is_empty() { None } else { Some(val) };
                match key {
                    "connection_id" => header.connection_id = opt,
                    "connection_fingerprint" => header.connection_fingerprint = opt,
                    "database" => header.database = opt,
                    "position" => {
                        header.position = opt.and_then(|v| v.parse().ok());
                    }
                    _ => {}
                }
            }
        } else {
            past_header = true;
            sql_lines.push(line);
        }
    }

    // Drop leading blank lines that separate headers from SQL.
    while sql_lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        sql_lines.remove(0);
    }

    (header, sql_lines.join("\n"))
}

fn write_file_content(header: &FileHeader, sql: &str) -> String {
    let mut out = String::new();
    let mut has_header = false;

    if let Some(id) = &header.connection_id {
        out.push_str(&format!("{HEADER_PREFIX}connection_id: {id}\n"));
        has_header = true;
    }
    if let Some(fp) = &header.connection_fingerprint {
        out.push_str(&format!("{HEADER_PREFIX}connection_fingerprint: {fp}\n"));
        has_header = true;
    }
    if let Some(db) = &header.database {
        out.push_str(&format!("{HEADER_PREFIX}database: {db}\n"));
        has_header = true;
    }
    if let Some(pos) = header.position {
        out.push_str(&format!("{HEADER_PREFIX}position: {pos}\n"));
        has_header = true;
    }
    if has_header {
        out.push('\n');
    }
    out.push_str(sql);
    out
}

// ── Path / filename helpers ───────────────────────────────────────────────────

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

/// Convert a `PathBuf` relative to `base` to a forward-slash string ID.
fn to_id(path: &Path, base: &Path) -> Result<String, AppError> {
    let rel = path
        .strip_prefix(base)
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn system_time_to_rfc3339(t: std::time::SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
}

// ── Connection helpers ────────────────────────────────────────────────────────

fn make_fingerprint(db_type: &str, host: &str, port: i64, database: &str) -> String {
    format!("{db_type}:{host}:{port}:{database}")
}

struct LocalConn {
    id: String,
    fingerprint: String,
}

async fn load_local_conns(sqlite: &SqlitePool) -> Result<Vec<LocalConn>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String,
        db_type: String,
        host: String,
        port: i64,
        database: String,
    }
    let rows: Vec<Row> =
        sqlx::query_as("SELECT id, db_type, host, port, database FROM connection_profiles")
            .fetch_all(sqlite)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| LocalConn {
            fingerprint: make_fingerprint(&r.db_type, &r.host, r.port, &r.database),
            id: r.id,
        })
        .collect())
}

async fn get_conn_fingerprint(
    sqlite: &SqlitePool,
    connection_id: &str,
) -> Result<Option<String>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        db_type: String,
        host: String,
        port: i64,
        database: String,
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT db_type, host, port, database FROM connection_profiles WHERE id = ?",
    )
    .bind(connection_id)
    .fetch_optional(sqlite)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(row.map(|r| make_fingerprint(&r.db_type, &r.host, r.port, &r.database)))
}

async fn load_mappings(sqlite: &SqlitePool) -> Result<HashMap<String, String>, AppError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(MAPPINGS_KEY)
            .fetch_optional(sqlite)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    match row {
        Some((v,)) => serde_json::from_str::<HashMap<String, String>>(&v)
            .map_err(|e| AppError::new("PARSE_ERROR", e.to_string())),
        None => Ok(HashMap::new()),
    }
}

async fn save_mappings(
    sqlite: &SqlitePool,
    mappings: &HashMap<String, String>,
) -> Result<(), AppError> {
    let json =
        serde_json::to_string(mappings).map_err(|e| AppError::new("SERIALISE_ERROR", e.to_string()))?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(MAPPINGS_KEY)
    .bind(json)
    .execute(sqlite)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

/// Returns `(resolved_local_connection_id, status)`.
/// Status is one of: "resolved" | "fingerprint_matched" | "unresolved" | "none".
fn resolve_connection(
    file_conn_id: &Option<String>,
    file_fp: &Option<String>,
    local: &[LocalConn],
    mappings: &HashMap<String, String>,
) -> (Option<String>, String) {
    let Some(file_id) = file_conn_id else {
        return (None, "none".to_string());
    };

    // Direct ID match.
    if local.iter().any(|c| c.id == *file_id) {
        return (Some(file_id.clone()), "resolved".to_string());
    }

    // Saved mapping (e.g. a team-mate's connection ID already mapped by this user).
    if let Some(local_id) = mappings.get(file_id) {
        if local.iter().any(|c| c.id == *local_id) {
            return (Some(local_id.clone()), "resolved".to_string());
        }
    }

    // Fingerprint match.
    if let Some(fp) = file_fp {
        if let Some(conn) = local.iter().find(|c| c.fingerprint == *fp) {
            return (Some(conn.id.clone()), "fingerprint_matched".to_string());
        }
    }

    (None, "unresolved".to_string())
}

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug, Clone)]
pub struct FileQuery {
    pub id: String,
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
    pub name: String,
    pub sql: String,
    #[serde(rename = "connectionId")]
    pub connection_id: Option<String>,
    #[serde(rename = "connectionStatus")]
    pub connection_status: String,
    #[serde(rename = "fileConnectionId")]
    pub file_connection_id: Option<String>,
    #[serde(rename = "fileFingerprint")]
    pub file_fingerprint: Option<String>,
    pub database: Option<String>,
    pub position: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct FileQueryFolder {
    pub id: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub name: String,
    pub position: i64,
}

#[derive(Serialize, Debug)]
pub struct FileQueryListResult {
    pub folders: Vec<FileQueryFolder>,
    pub queries: Vec<FileQuery>,
}

#[derive(Deserialize, Debug)]
pub struct PositionUpdate {
    pub id: String,
    pub position: i64,
}

// ── Directory scanner ─────────────────────────────────────────────────────────

fn scan_dir(
    dir: &Path,
    base: &Path,
    local: &[LocalConn],
    mappings: &HashMap<String, String>,
    new_fp_mappings: &mut HashMap<String, String>,
    folders: &mut Vec<FileQueryFolder>,
    queries: &mut Vec<FileQuery>,
) -> Result<(), AppError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };

    let order = read_order_file(dir);
    let order_pos = |name: &str| -> i64 {
        order.iter().position(|o| o == name)
            .map(|i| i as i64)
            .unwrap_or(order.len() as i64)
    };

    let mut subdirs: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = path.metadata() else { continue };
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        if file_name == ORDER_FILENAME {
            continue;
        }

        if meta.is_dir() {
            let name = file_name.into_owned();
            let id = to_id(&path, base)?;
            let parent_id = path
                .parent()
                .and_then(|p| to_id(p, base).ok())
                .filter(|s| !s.is_empty());
            let position = order_pos(&name);
            folders.push(FileQueryFolder { id, parent_id, name, position });
            subdirs.push(path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            let filename = file_name.into_owned(); // e.g. "monthly.sql"
            let id = to_id(&path, base)?;
            let folder_id = path
                .parent()
                .and_then(|p| to_id(p, base).ok())
                .filter(|s| !s.is_empty());
            let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let (header, sql) = parse_header(&content);
            let (conn_id, conn_status) =
                resolve_connection(&header.connection_id, &header.connection_fingerprint, local, mappings);

            if conn_status == "fingerprint_matched" {
                if let (Some(fid), Some(lid)) = (&header.connection_id, &conn_id) {
                    new_fp_mappings.insert(fid.clone(), lid.clone());
                }
            }

            let created_at = system_time_to_rfc3339(
                meta.created().unwrap_or_else(|_| meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)),
            );
            let updated_at = system_time_to_rfc3339(
                meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            );

            let position = if order.iter().any(|o| o == &filename) {
                order_pos(&filename)
            } else {
                header.position.unwrap_or(order.len() as i64)
            };

            queries.push(FileQuery {
                id,
                folder_id,
                name,
                sql,
                connection_id: conn_id,
                connection_status: conn_status,
                file_connection_id: header.connection_id,
                file_fingerprint: header.connection_fingerprint,
                database: header.database,
                position,
                created_at,
                updated_at,
            });
        }
    }

    for subdir in subdirs {
        scan_dir(&subdir, base, local, mappings, new_fp_mappings, folders, queries)?;
    }

    Ok(())
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn file_saved_queries_get_dir(
    sqlite: State<'_, SqlitePool>,
) -> Result<String, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    Ok(dir.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn file_saved_queries_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<FileQueryListResult, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    std::fs::create_dir_all(&dir).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    let local = load_local_conns(sqlite.inner()).await?;
    let mut mappings = load_mappings(sqlite.inner()).await?;
    let mut new_fp_mappings: HashMap<String, String> = HashMap::new();
    let mut folders: Vec<FileQueryFolder> = Vec::new();
    let mut queries: Vec<FileQuery> = Vec::new();

    scan_dir(&dir, &dir, &local, &mappings, &mut new_fp_mappings, &mut folders, &mut queries)?;

    if !new_fp_mappings.is_empty() {
        mappings.extend(new_fp_mappings);
        save_mappings(sqlite.inner(), &mappings).await?;
    }

    folders.sort_by(|a, b| a.position.cmp(&b.position).then(a.name.cmp(&b.name)));
    queries.sort_by(|a, b| a.position.cmp(&b.position).then(a.name.cmp(&b.name)));

    Ok(FileQueryListResult { folders, queries })
}

#[tauri::command]
pub async fn file_saved_queries_create(
    sqlite: State<'_, SqlitePool>,
    folder_id: Option<String>,
    name: String,
    sql: String,
    connection_id: Option<String>,
    database: Option<String>,
) -> Result<FileQuery, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;

    let target_dir = match &folder_id {
        Some(fid) => dir.join(fid),
        None => dir.clone(),
    };
    std::fs::create_dir_all(&target_dir).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    let safe_name = sanitize_filename(&name);
    let file_path = target_dir.join(format!("{safe_name}.sql"));

    if file_path.exists() {
        return Err(AppError::new(
            "CONFLICT",
            format!("A query named '{name}' already exists in this location"),
        ));
    }

    // Count existing .sql files to auto-assign position.
    let position = std::fs::read_dir(&target_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.path().extension().and_then(|x| x.to_str()) == Some("sql")
                })
                .count() as i64
        })
        .unwrap_or(0);

    let fingerprint = if let Some(ref cid) = connection_id {
        get_conn_fingerprint(sqlite.inner(), cid).await.ok().flatten()
    } else {
        None
    };

    let header = FileHeader {
        connection_id: connection_id.clone(),
        connection_fingerprint: fingerprint.clone(),
        database: database.clone(),
        position: Some(position),
    };

    std::fs::write(&file_path, write_file_content(&header, &sql))
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    let meta = file_path.metadata().map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    let id = to_id(&file_path, &dir)?;
    let ts = system_time_to_rfc3339(meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH));

    let conn_status = if connection_id.is_some() { "resolved" } else { "none" }.to_string();

    Ok(FileQuery {
        id,
        folder_id,
        name: safe_name,
        sql,
        connection_id: connection_id.clone(),
        connection_status: conn_status,
        file_connection_id: connection_id,
        file_fingerprint: fingerprint,
        database,
        position,
        created_at: ts.clone(),
        updated_at: ts,
    })
}

#[tauri::command]
pub async fn file_saved_queries_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    name: String,
    sql: String,
    connection_id: Option<String>,
    folder_id: Option<String>,
    database: Option<String>,
) -> Result<FileQuery, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let old_path = dir.join(&id);

    if !old_path.exists() {
        return Err(AppError::new("NOT_FOUND", format!("Query '{id}' not found")));
    }

    // Preserve existing position.
    let existing = std::fs::read_to_string(&old_path)
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    let (old_header, _) = parse_header(&existing);

    let target_dir = match &folder_id {
        Some(fid) => dir.join(fid),
        None => dir.clone(),
    };
    std::fs::create_dir_all(&target_dir).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    let safe_name = sanitize_filename(&name);
    let new_path = target_dir.join(format!("{safe_name}.sql"));

    if new_path != old_path && new_path.exists() {
        return Err(AppError::new(
            "CONFLICT",
            format!("A query named '{name}' already exists in this location"),
        ));
    }

    let fingerprint = if let Some(ref cid) = connection_id {
        get_conn_fingerprint(sqlite.inner(), cid).await.ok().flatten()
    } else {
        None
    };

    let header = FileHeader {
        connection_id: connection_id.clone(),
        connection_fingerprint: fingerprint.clone(),
        database: database.clone(),
        position: old_header.position,
    };

    // Write to the target path first.
    std::fs::write(&new_path, write_file_content(&header, &sql))
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    // Remove the old file only if the path changed.
    if new_path != old_path {
        let _ = std::fs::remove_file(&old_path);
    }

    let meta = new_path.metadata().map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    let new_id = to_id(&new_path, &dir)?;
    let actual_folder_id = new_path
        .parent()
        .and_then(|p| to_id(p, &dir).ok())
        .filter(|s| !s.is_empty());

    let local = load_local_conns(sqlite.inner()).await?;
    let mappings = load_mappings(sqlite.inner()).await?;
    let (resolved_id, conn_status) =
        resolve_connection(&connection_id, &fingerprint, &local, &mappings);

    Ok(FileQuery {
        id: new_id,
        folder_id: actual_folder_id,
        name: safe_name,
        sql,
        connection_id: resolved_id,
        connection_status: conn_status,
        file_connection_id: connection_id,
        file_fingerprint: fingerprint,
        database,
        position: old_header.position.unwrap_or(0),
        created_at: system_time_to_rfc3339(
            meta.created().unwrap_or_else(|_| meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)),
        ),
        updated_at: system_time_to_rfc3339(
            meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        ),
    })
}

#[tauri::command]
pub async fn file_saved_queries_delete(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let path = dir.join(&id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    }
    Ok(())
}

/// Batch-update the `position` header in multiple SQL files without touching
/// the SQL body or any other headers.
#[tauri::command]
pub async fn file_saved_queries_update_positions(
    sqlite: State<'_, SqlitePool>,
    updates: Vec<PositionUpdate>,
) -> Result<(), AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    for u in updates {
        let path = dir.join(&u.id);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
        let (mut header, sql) = parse_header(&content);
        header.position = Some(u.position);
        std::fs::write(&path, write_file_content(&header, &sql))
            .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    }
    Ok(())
}

/// Write the `_order` file for a directory, defining the sort order of all
/// direct children (queries and folders). Items not listed fall to the end,
/// sorted alphabetically.
#[tauri::command]
pub async fn file_saved_queries_update_order(
    sqlite: State<'_, SqlitePool>,
    parent_id: Option<String>,
    items: Vec<String>,
) -> Result<(), AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let target_dir = match &parent_id {
        Some(pid) => dir.join(pid),
        None => dir.clone(),
    };
    write_order_file(&target_dir, &items)
}

#[tauri::command]
pub async fn file_saved_queries_create_folder(
    sqlite: State<'_, SqlitePool>,
    parent_id: Option<String>,
    name: String,
) -> Result<FileQueryFolder, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let safe_name = sanitize_filename(&name);
    let target = match &parent_id {
        Some(pid) => dir.join(pid).join(&safe_name),
        None => dir.join(&safe_name),
    };
    if target.exists() {
        return Err(AppError::new(
            "CONFLICT",
            format!("A folder named '{name}' already exists"),
        ));
    }
    std::fs::create_dir_all(&target).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    let id = to_id(&target, &dir)?;
    Ok(FileQueryFolder { id, parent_id, name: safe_name, position: 0 })
}

#[tauri::command]
pub async fn file_saved_queries_delete_folder(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let path = dir.join(&id);
    if path.exists() {
        std::fs::remove_dir_all(&path).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn file_saved_queries_rename_folder(
    sqlite: State<'_, SqlitePool>,
    id: String,
    name: String,
) -> Result<FileQueryFolder, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let old_path = dir.join(&id);
    let parent = old_path
        .parent()
        .ok_or_else(|| AppError::new("IO_ERROR", "Cannot determine parent directory"))?;
    let safe_name = sanitize_filename(&name);
    let new_path = parent.join(&safe_name);

    if new_path != old_path && new_path.exists() {
        return Err(AppError::new(
            "CONFLICT",
            format!("A folder named '{name}' already exists"),
        ));
    }
    std::fs::rename(&old_path, &new_path)
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    let new_id = to_id(&new_path, &dir)?;
    let parent_id = new_path
        .parent()
        .and_then(|p| to_id(p, &dir).ok())
        .filter(|s| !s.is_empty());

    Ok(FileQueryFolder { id: new_id, parent_id, name: safe_name, position: 0 })
}

#[tauri::command]
pub async fn file_saved_queries_move_folder(
    sqlite: State<'_, SqlitePool>,
    id: String,
    new_parent_id: Option<String>,
) -> Result<FileQueryFolder, AppError> {
    let dir = queries_dir(sqlite.inner()).await?;
    let src_path = dir.join(&id);

    // Prevent moving into itself or a descendant.
    if let Some(ref np) = new_parent_id {
        if np == &id || np.starts_with(&format!("{}/", id)) {
            return Err(AppError::new(
                "INVALID_INPUT",
                "Cannot move a folder into itself or its descendant",
            ));
        }
    }

    let folder_name = src_path
        .file_name()
        .ok_or_else(|| AppError::new("IO_ERROR", "Invalid folder path"))?
        .to_string_lossy()
        .into_owned();

    let dest_parent = match &new_parent_id {
        Some(np) => dir.join(np),
        None => dir.clone(),
    };
    let dest_path = dest_parent.join(&folder_name);

    if dest_path == src_path {
        let parent_id = src_path
            .parent()
            .and_then(|p| to_id(p, &dir).ok())
            .filter(|s| !s.is_empty());
        return Ok(FileQueryFolder { id, parent_id, name: folder_name, position: 0 });
    }

    if dest_path.exists() {
        return Err(AppError::new(
            "CONFLICT",
            format!("A folder named '{folder_name}' already exists at the destination"),
        ));
    }

    // Remove from old parent _order.
    let old_parent = src_path
        .parent()
        .ok_or_else(|| AppError::new("IO_ERROR", "Cannot determine source parent"))?;
    let mut old_order = read_order_file(old_parent);
    old_order.retain(|e| e != &folder_name);
    write_order_file(old_parent, &old_order)?;

    // Move the directory.
    std::fs::create_dir_all(&dest_parent).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    std::fs::rename(&src_path, &dest_path).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;

    // Append to new parent _order.
    let mut new_order = read_order_file(&dest_parent);
    if !new_order.contains(&folder_name) {
        new_order.push(folder_name.clone());
    }
    write_order_file(&dest_parent, &new_order)?;

    let new_id = to_id(&dest_path, &dir)?;
    let parent_id = dest_path
        .parent()
        .and_then(|p| to_id(p, &dir).ok())
        .filter(|s| !s.is_empty());

    Ok(FileQueryFolder { id: new_id, parent_id, name: folder_name, position: 0 })
}

/// Persist a mapping from a foreign connection ID (seen in a cloned file) to
/// the user's local connection ID.  Subsequent loads resolve automatically.
#[tauri::command]
pub async fn file_saved_queries_assign_connection(
    sqlite: State<'_, SqlitePool>,
    file_connection_id: String,
    local_connection_id: String,
) -> Result<(), AppError> {
    let mut mappings = load_mappings(sqlite.inner()).await?;
    mappings.insert(file_connection_id, local_connection_id);
    save_mappings(sqlite.inner(), &mappings).await
}

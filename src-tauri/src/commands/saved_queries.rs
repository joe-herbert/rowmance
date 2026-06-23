/// Tauri commands for managing saved queries and their folders.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug, Clone)]
pub struct SavedQueryFolder {
    pub id: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub position: i64,
}

#[derive(Deserialize, Debug)]
pub struct SavedQueryFolderInput {
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub position: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
pub struct SavedQuery {
    pub id: String,
    #[serde(rename = "connectionId")]
    pub connection_id: Option<String>,
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
    pub name: String,
    pub sql: String,
    pub position: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Deserialize, Debug)]
pub struct SavedQueryInput {
    #[serde(rename = "connectionId")]
    pub connection_id: Option<String>,
    #[serde(rename = "folderId")]
    pub folder_id: Option<String>,
    pub name: String,
    pub sql: String,
    pub position: Option<i64>,
}

// ── SQLite row types ──────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct SavedQueryFolderRow {
    id: String,
    name: String,
    parent_id: Option<String>,
    position: i64,
}

impl From<SavedQueryFolderRow> for SavedQueryFolder {
    fn from(r: SavedQueryFolderRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            parent_id: r.parent_id,
            position: r.position,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SavedQueryRow {
    id: String,
    connection_id: Option<String>,
    folder_id: Option<String>,
    name: String,
    sql: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

impl From<SavedQueryRow> for SavedQuery {
    fn from(r: SavedQueryRow) -> Self {
        Self {
            id: r.id,
            connection_id: r.connection_id,
            folder_id: r.folder_id,
            name: r.name,
            sql: r.sql,
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// ── Folder commands ───────────────────────────────────────────────────────────

/// List all saved query folders ordered by position then name.
#[tauri::command]
pub async fn saved_queries_list_folders(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<SavedQueryFolder>, AppError> {
    let rows = sqlx::query_as::<_, SavedQueryFolderRow>(
        "SELECT * FROM saved_query_folders ORDER BY position, name",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(SavedQueryFolder::from).collect())
}

/// Create a new saved query folder.
#[tauri::command]
pub async fn saved_queries_create_folder(
    sqlite: State<'_, SqlitePool>,
    input: SavedQueryFolderInput,
) -> Result<SavedQueryFolder, AppError> {
    let id = Uuid::new_v4().to_string();
    let position = input.position.unwrap_or(0);

    sqlx::query!(
        "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, ?, ?, ?)",
        id,
        input.name,
        input.parent_id,
        position
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, SavedQueryFolderRow>(
        "SELECT * FROM saved_query_folders WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(SavedQueryFolder::from(row))
}

/// Update an existing saved query folder.
#[tauri::command]
pub async fn saved_queries_update_folder(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: SavedQueryFolderInput,
) -> Result<SavedQueryFolder, AppError> {
    let position = input.position.unwrap_or(0);

    sqlx::query!(
        "UPDATE saved_query_folders SET name = ?, parent_id = ?, position = ? WHERE id = ?",
        input.name,
        input.parent_id,
        position,
        id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, SavedQueryFolderRow>(
        "SELECT * FROM saved_query_folders WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(SavedQueryFolder::from(row))
}

/// Delete a saved query folder by id.
#[tauri::command]
pub async fn saved_queries_delete_folder(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM saved_query_folders WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

// ── Query commands ────────────────────────────────────────────────────────────

/// List saved queries. If folder_id is provided, filter by that folder.
#[tauri::command]
pub async fn saved_queries_list(
    sqlite: State<'_, SqlitePool>,
    folder_id: Option<String>,
) -> Result<Vec<SavedQuery>, AppError> {
    let rows = match folder_id {
        Some(fid) => {
            sqlx::query_as::<_, SavedQueryRow>(
                "SELECT * FROM saved_queries WHERE folder_id = ? ORDER BY position, name",
            )
            .bind(fid)
            .fetch_all(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
        }
        None => {
            sqlx::query_as::<_, SavedQueryRow>(
                "SELECT * FROM saved_queries ORDER BY position, name",
            )
            .fetch_all(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
        }
    };

    Ok(rows.into_iter().map(SavedQuery::from).collect())
}

/// Create a new saved query.
#[tauri::command]
pub async fn saved_queries_create(
    sqlite: State<'_, SqlitePool>,
    input: SavedQueryInput,
) -> Result<SavedQuery, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let position = if let Some(pos) = input.position {
        pos
    } else {
        let max_pos: Option<i64> = match &input.folder_id {
            Some(fid) => sqlx::query_scalar!(
                "SELECT MAX(position) FROM saved_queries WHERE folder_id = ?",
                fid
            )
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?,
            None => sqlx::query_scalar!(
                "SELECT MAX(position) FROM saved_queries WHERE folder_id IS NULL"
            )
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?,
        };
        max_pos.map(|m| m + 1).unwrap_or(0)
    };

    sqlx::query!(
        r#"
        INSERT INTO saved_queries (id, connection_id, folder_id, name, sql, position, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        id,
        input.connection_id,
        input.folder_id,
        input.name,
        input.sql,
        position,
        now,
        now
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, SavedQueryRow>("SELECT * FROM saved_queries WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(SavedQuery::from(row))
}

/// Update an existing saved query.
#[tauri::command]
pub async fn saved_queries_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: SavedQueryInput,
) -> Result<SavedQuery, AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(position) = input.position {
        sqlx::query!(
            r#"
            UPDATE saved_queries
            SET connection_id = ?, folder_id = ?, name = ?, sql = ?, position = ?, updated_at = ?
            WHERE id = ?
            "#,
            input.connection_id,
            input.folder_id,
            input.name,
            input.sql,
            position,
            now,
            id
        )
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    } else {
        sqlx::query!(
            r#"
            UPDATE saved_queries
            SET connection_id = ?, folder_id = ?, name = ?, sql = ?, updated_at = ?
            WHERE id = ?
            "#,
            input.connection_id,
            input.folder_id,
            input.name,
            input.sql,
            now,
            id
        )
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }

    let row =
        sqlx::query_as::<_, SavedQueryRow>("SELECT * FROM saved_queries WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(SavedQuery::from(row))
}

/// Delete a saved query by id.
#[tauri::command]
pub async fn saved_queries_delete(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM saved_queries WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations").run(&pool).await.unwrap();
        pool
    }

    // ── Deserialization ───────────────────────────────────────────────────────

    #[test]
    fn saved_query_folder_input_deserializes_camel_case_parent_id() {
        let json = r#"{"name":"My Folder","parentId":"abc-123","position":2}"#;
        let input: SavedQueryFolderInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "My Folder");
        assert_eq!(input.parent_id.as_deref(), Some("abc-123"));
        assert_eq!(input.position, Some(2));
    }

    #[test]
    fn saved_query_folder_input_accepts_null_parent_id() {
        let json = r#"{"name":"Root Folder","parentId":null}"#;
        let input: SavedQueryFolderInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "Root Folder");
        assert!(input.parent_id.is_none());
        assert!(input.position.is_none());
    }

    #[test]
    fn saved_query_folder_input_parent_id_defaults_to_none_when_absent() {
        let json = r#"{"name":"Folder Only"}"#;
        let input: SavedQueryFolderInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "Folder Only");
        assert!(input.parent_id.is_none());
        assert!(input.position.is_none());
    }

    #[test]
    fn saved_query_input_deserializes_camel_case_fields() {
        let json = r#"{"connectionId":"conn-1","folderId":"fold-2","name":"My Query","sql":"SELECT 1"}"#;
        let input: SavedQueryInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.connection_id.as_deref(), Some("conn-1"));
        assert_eq!(input.folder_id.as_deref(), Some("fold-2"));
        assert_eq!(input.name, "My Query");
        assert_eq!(input.sql, "SELECT 1");
    }

    #[test]
    fn saved_query_input_accepts_null_connection_and_folder() {
        let json = r#"{"connectionId":null,"folderId":null,"name":"Unfiled","sql":"SELECT 2"}"#;
        let input: SavedQueryInput = serde_json::from_str(json).unwrap();
        assert!(input.connection_id.is_none());
        assert!(input.folder_id.is_none());
    }

    // ── From conversions ──────────────────────────────────────────────────────

    #[test]
    fn saved_query_folder_from_row_preserves_all_fields() {
        let row = SavedQueryFolderRow {
            id: "f-1".to_owned(),
            name: "Production".to_owned(),
            parent_id: Some("f-0".to_owned()),
            position: 3,
        };
        let folder = SavedQueryFolder::from(row);
        assert_eq!(folder.id, "f-1");
        assert_eq!(folder.name, "Production");
        assert_eq!(folder.parent_id.as_deref(), Some("f-0"));
        assert_eq!(folder.position, 3);
    }

    #[test]
    fn saved_query_from_row_preserves_all_fields() {
        let row = SavedQueryRow {
            id: "q-1".to_owned(),
            connection_id: Some("c-1".to_owned()),
            folder_id: Some("f-1".to_owned()),
            name: "Get users".to_owned(),
            sql: "SELECT * FROM users".to_owned(),
            created_at: "2024-01-01T00:00:00Z".to_owned(),
            updated_at: "2024-01-02T00:00:00Z".to_owned(),
        };
        let query = SavedQuery::from(row);
        assert_eq!(query.id, "q-1");
        assert_eq!(query.connection_id.as_deref(), Some("c-1"));
        assert_eq!(query.folder_id.as_deref(), Some("f-1"));
        assert_eq!(query.name, "Get users");
        assert_eq!(query.sql, "SELECT * FROM users");
    }

    // ── Database round-trip ───────────────────────────────────────────────────

    #[tokio::test]
    async fn list_folders_returns_empty_initially() {
        let pool = setup_db().await;
        let rows = sqlx::query_as::<_, SavedQueryFolderRow>(
            "SELECT * FROM saved_query_folders ORDER BY position, name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn create_folder_round_trips() {
        let pool = setup_db().await;
        let id = "fld-1";

        sqlx::query(
            "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, ?, NULL, 0)",
        )
        .bind(id)
        .bind("My Folder")
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query_as::<_, SavedQueryFolderRow>(
            "SELECT * FROM saved_query_folders WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.id, id);
        assert_eq!(row.name, "My Folder");
        assert!(row.parent_id.is_none());
        assert_eq!(row.position, 0);
    }

    #[tokio::test]
    async fn create_query_round_trips() {
        let pool = setup_db().await;
        let id = "q-rt";
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO saved_queries (id, connection_id, folder_id, name, sql, created_at, updated_at)
               VALUES (?, NULL, NULL, ?, ?, ?, ?)"#,
        )
        .bind(id)
        .bind("My Query")
        .bind("SELECT 1")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row =
            sqlx::query_as::<_, SavedQueryRow>("SELECT * FROM saved_queries WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.id, id);
        assert_eq!(row.name, "My Query");
        assert_eq!(row.sql, "SELECT 1");
        assert!(row.connection_id.is_none());
        assert!(row.folder_id.is_none());
    }

    #[tokio::test]
    async fn delete_folder_removes_it() {
        let pool = setup_db().await;
        let id = "fld-del";

        sqlx::query(
            "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, 'Del', NULL, 0)",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM saved_query_folders WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let row = sqlx::query_as::<_, SavedQueryFolderRow>(
            "SELECT * FROM saved_query_folders WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(row.is_none());
    }

    // ── Additional saved_query_folders tests ──────────────────────────────────

    #[tokio::test]
    async fn update_folder_changes_name() {
        let pool = setup_db().await;
        let id = "fld-upd";

        sqlx::query(
            "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, ?, NULL, 0)",
        )
        .bind(id)
        .bind("Old")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "UPDATE saved_query_folders SET name = ?, parent_id = ?, position = ? WHERE id = ?",
        )
        .bind("New")
        .bind(None::<String>)
        .bind(0_i64)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query_as::<_, SavedQueryFolderRow>(
            "SELECT * FROM saved_query_folders WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.name, "New");
    }

    #[tokio::test]
    async fn list_folders_returns_all() {
        let pool = setup_db().await;

        for (id, name) in &[("fla-1", "Folder 1"), ("fla-2", "Folder 2"), ("fla-3", "Folder 3")] {
            sqlx::query(
                "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, ?, NULL, 0)",
            )
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        }

        let rows = sqlx::query_as::<_, SavedQueryFolderRow>(
            "SELECT * FROM saved_query_folders",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 3);
    }

    #[tokio::test]
    async fn list_folders_orders_by_position_then_name() {
        let pool = setup_db().await;

        // Insert with positions 2, 0, 1 and names C, A, B respectively.
        let entries: &[(&str, &str, i64)] = &[
            ("flo-1", "C", 2),
            ("flo-2", "A", 0),
            ("flo-3", "B", 1),
        ];
        for (id, name, position) in entries {
            sqlx::query(
                "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, ?, NULL, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(position)
            .execute(&pool)
            .await
            .unwrap();
        }

        let rows = sqlx::query_as::<_, SavedQueryFolderRow>(
            "SELECT * FROM saved_query_folders ORDER BY position, name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name, "A");
        assert_eq!(rows[0].position, 0);
        assert_eq!(rows[1].name, "B");
        assert_eq!(rows[1].position, 1);
        assert_eq!(rows[2].name, "C");
        assert_eq!(rows[2].position, 2);
    }

    // ── Additional saved_queries tests ────────────────────────────────────────

    #[tokio::test]
    async fn update_query_changes_sql_and_name() {
        let pool = setup_db().await;
        let id = "q-upd";
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO saved_queries (id, connection_id, folder_id, name, sql, created_at, updated_at)
               VALUES (?, NULL, NULL, ?, ?, ?, ?)"#,
        )
        .bind(id)
        .bind("Old Name")
        .bind("SELECT 1")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"UPDATE saved_queries
               SET connection_id = ?, folder_id = ?, name = ?, sql = ?, updated_at = ?
               WHERE id = ?"#,
        )
        .bind(None::<String>)
        .bind(None::<String>)
        .bind("New Name")
        .bind("SELECT 2")
        .bind(&updated_at)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        let row =
            sqlx::query_as::<_, SavedQueryRow>("SELECT * FROM saved_queries WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.name, "New Name");
        assert_eq!(row.sql, "SELECT 2");
    }

    #[tokio::test]
    async fn delete_query_removes_it() {
        let pool = setup_db().await;
        let id = "q-del";
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO saved_queries (id, connection_id, folder_id, name, sql, created_at, updated_at)
               VALUES (?, NULL, NULL, 'ToDelete', 'SELECT 1', ?, ?)"#,
        )
        .bind(id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM saved_queries WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let row =
            sqlx::query_as::<_, SavedQueryRow>("SELECT * FROM saved_queries WHERE id = ?")
                .bind(id)
                .fetch_optional(&pool)
                .await
                .unwrap();

        assert!(row.is_none());
    }

    #[tokio::test]
    async fn list_queries_by_folder_id() {
        let pool = setup_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        let folder_id = "fld-qf";

        sqlx::query(
            "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, 'QF', NULL, 0)",
        )
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

        // Two queries in the folder, one without folder.
        for (id, fid) in &[
            ("q-fld-1", Some(folder_id)),
            ("q-fld-2", Some(folder_id)),
            ("q-fld-3", None),
        ] {
            sqlx::query(
                r#"INSERT INTO saved_queries (id, connection_id, folder_id, name, sql, created_at, updated_at)
                   VALUES (?, NULL, ?, 'Q', 'SELECT 1', ?, ?)"#,
            )
            .bind(id)
            .bind(*fid)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        }

        let rows = sqlx::query_as::<_, SavedQueryRow>(
            "SELECT * FROM saved_queries WHERE folder_id = ? ORDER BY name",
        )
        .bind(folder_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn list_all_queries_when_folder_id_is_none() {
        let pool = setup_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        let folder_id = "fld-all";

        sqlx::query(
            "INSERT INTO saved_query_folders (id, name, parent_id, position) VALUES (?, 'All', NULL, 0)",
        )
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

        // Three queries: two with folder, one without.
        for (id, fid) in &[
            ("q-all-1", Some(folder_id)),
            ("q-all-2", Some(folder_id)),
            ("q-all-3", None),
        ] {
            sqlx::query(
                r#"INSERT INTO saved_queries (id, connection_id, folder_id, name, sql, created_at, updated_at)
                   VALUES (?, NULL, ?, 'Q', 'SELECT 1', ?, ?)"#,
            )
            .bind(id)
            .bind(*fid)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        }

        let rows =
            sqlx::query_as::<_, SavedQueryRow>("SELECT * FROM saved_queries ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert_eq!(rows.len(), 3);
    }
}

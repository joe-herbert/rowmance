/// Tauri commands for managing tags and their attachment to connections.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::db::models::TagRow;
use crate::error::AppError;

#[derive(Debug, Serialize, Clone)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

impl From<TagRow> for Tag {
    fn from(r: TagRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            color: r.color,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TagInput {
    pub name: String,
    pub color: Option<String>,
}

/// Fetch all tags attached to a connection, ordered the same way as `tags_list`.
pub(crate) async fn fetch_tags_for_connection(
    pool: &SqlitePool,
    connection_id: &str,
) -> Result<Vec<Tag>, AppError> {
    let rows = sqlx::query_as::<_, TagRow>(
        r#"
        SELECT tags.* FROM tags
        JOIN connection_tags ON connection_tags.tag_id = tags.id
        WHERE connection_tags.connection_id = ?
        ORDER BY tags.position, tags.name
        "#,
    )
    .bind(connection_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(Tag::from).collect())
}

/// List all tags.
#[tauri::command]
pub async fn tags_list(sqlite: State<'_, SqlitePool>) -> Result<Vec<Tag>, AppError> {
    let rows = sqlx::query_as::<_, TagRow>("SELECT * FROM tags ORDER BY position, name")
        .fetch_all(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(Tag::from).collect())
}

/// Create a new tag.
#[tauri::command]
pub async fn tags_create(
    sqlite: State<'_, SqlitePool>,
    input: TagInput,
) -> Result<Tag, AppError> {
    let id = Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO tags (id, name, color, position) VALUES (?, ?, ?, 0)")
        .bind(&id)
        .bind(&input.name)
        .bind(&input.color)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, TagRow>("SELECT * FROM tags WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(Tag::from(row))
}

/// Update an existing tag's name and color.
#[tauri::command]
pub async fn tags_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: TagInput,
) -> Result<Tag, AppError> {
    sqlx::query("UPDATE tags SET name = ?, color = ? WHERE id = ?")
        .bind(&input.name)
        .bind(&input.color)
        .bind(&id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, TagRow>("SELECT * FROM tags WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(Tag::from(row))
}

/// Reorder tags. `ids` is the full list of tag ids in their new display order.
async fn reorder_tags(pool: &SqlitePool, ids: &[String]) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    for (i, id) in ids.iter().enumerate() {
        let position = i as i64;
        sqlx::query!("UPDATE tags SET position = ? WHERE id = ?", position, id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn tags_reorder(sqlite: State<'_, SqlitePool>, ids: Vec<String>) -> Result<(), AppError> {
    reorder_tags(sqlite.inner(), &ids).await
}

/// Delete a tag. Cascades to remove it from any connections it was attached to.
#[tauri::command]
pub async fn tags_delete(sqlite: State<'_, SqlitePool>, id: String) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM tags WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

/// Replace the full set of tags attached to a connection.
async fn set_connection_tags(
    pool: &SqlitePool,
    id: &str,
    tag_ids: &[String],
) -> Result<crate::commands::connections::ConnectionProfile, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    sqlx::query!("DELETE FROM connection_tags WHERE connection_id = ?", id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    for tag_id in tag_ids {
        sqlx::query!(
            "INSERT INTO connection_tags (connection_id, tag_id) VALUES (?, ?)",
            id,
            tag_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    crate::commands::connections::fetch_connection_profile(pool, id).await
}

#[tauri::command]
pub async fn connections_set_tags(
    sqlite: State<'_, SqlitePool>,
    id: String,
    tag_ids: Vec<String>,
) -> Result<crate::commands::connections::ConnectionProfile, AppError> {
    set_connection_tags(sqlite.inner(), &id, &tag_ids).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        pool
    }

    async fn insert_profile(pool: &SqlitePool, id: &str, name: &str) {
        sqlx::query(
            r#"INSERT INTO connection_profiles
               (id, name, db_type, host, port, database, username, read_only,
                ssh_enabled, ssl_enabled, pool_max, created_at, updated_at)
               VALUES (?, ?, 'postgres', 'localhost', 5432, 'db', 'user', 0,
                       0, 0, 5, '2024-01-01', '2024-01-01')"#,
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_tag(pool: &SqlitePool, id: &str, name: &str) {
        sqlx::query("INSERT INTO tags (id, name, color, position) VALUES (?, ?, NULL, 0)")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_tag_cascades_from_connection() {
        let pool = setup_db().await;
        insert_profile(&pool, "conn-1", "Server").await;
        insert_tag(&pool, "tag-1", "prod").await;

        sqlx::query("INSERT INTO connection_tags (connection_id, tag_id) VALUES ('conn-1', 'tag-1')")
            .execute(&pool)
            .await
            .unwrap();

        let tags = fetch_tags_for_connection(&pool, "conn-1").await.unwrap();
        assert_eq!(tags.len(), 1);

        sqlx::query!("DELETE FROM tags WHERE id = 'tag-1'")
            .execute(&pool)
            .await
            .unwrap();

        let tags = fetch_tags_for_connection(&pool, "conn-1").await.unwrap();
        assert!(tags.is_empty());
    }

    #[tokio::test]
    async fn reorder_updates_positions() {
        let pool = setup_db().await;
        insert_tag(&pool, "tag-a", "a").await;
        insert_tag(&pool, "tag-b", "b").await;
        insert_tag(&pool, "tag-c", "c").await;

        reorder_tags(
            &pool,
            &["tag-c".to_string(), "tag-a".to_string(), "tag-b".to_string()],
        )
        .await
        .unwrap();

        let rows = sqlx::query_as::<_, TagRow>("SELECT * FROM tags ORDER BY position, name")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["tag-c", "tag-a", "tag-b"]
        );
    }

    #[tokio::test]
    async fn set_tags_replaces_existing_associations() {
        let pool = setup_db().await;
        insert_profile(&pool, "conn-1", "Server").await;
        insert_tag(&pool, "tag-a", "a").await;
        insert_tag(&pool, "tag-b", "b").await;

        let profile = set_connection_tags(
            &pool,
            "conn-1",
            &["tag-a".to_string(), "tag-b".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(profile.tags.len(), 2);

        let profile = set_connection_tags(&pool, "conn-1", &["tag-b".to_string()])
            .await
            .unwrap();
        assert_eq!(profile.tags.len(), 1);
        assert_eq!(profile.tags[0].id, "tag-b");
    }
}

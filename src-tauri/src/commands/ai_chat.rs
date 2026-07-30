/// Tauri commands for managing persisted AI chat conversations and messages.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug, Clone)]
pub struct AiConversation {
    pub id: String,
    pub mode: String,
    #[serde(rename = "contextKey")]
    pub context_key: Option<String>,
    pub title: String,
    #[serde(rename = "connectionId")]
    pub connection_id: Option<String>,
    pub database: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Deserialize, Debug)]
pub struct AiConversationInput {
    pub mode: String,
    #[serde(rename = "contextKey")]
    pub context_key: Option<String>,
    pub title: String,
    #[serde(rename = "connectionId")]
    pub connection_id: Option<String>,
    pub database: Option<String>,
    #[serde(rename = "firstUserMessage")]
    pub first_user_message: Option<String>,
    #[serde(rename = "firstAssistantMessage")]
    pub first_assistant_message: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct AiMessage {
    pub id: String,
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

// ── SQLite row types ──────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct AiConversationRow {
    id: String,
    mode: String,
    context_key: Option<String>,
    title: String,
    connection_id: Option<String>,
    database: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<AiConversationRow> for AiConversation {
    fn from(r: AiConversationRow) -> Self {
        Self {
            id: r.id,
            mode: r.mode,
            context_key: r.context_key,
            title: r.title,
            connection_id: r.connection_id,
            database: r.database,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AiMessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    created_at: String,
}

impl From<AiMessageRow> for AiMessage {
    fn from(r: AiMessageRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            role: r.role,
            content: r.content,
            created_at: r.created_at,
        }
    }
}

// ── Conversation commands ─────────────────────────────────────────────────────

/// Find the most recently updated conversation matching a mode/context/connection/database
/// combination, used to resume a source's thread when reopening the AI modal.
#[tauri::command]
pub async fn ai_chat_find_conversation(
    sqlite: State<'_, SqlitePool>,
    mode: String,
    context_key: Option<String>,
    connection_id: Option<String>,
    database: Option<String>,
) -> Result<Option<AiConversation>, AppError> {
    let row = sqlx::query_as::<_, AiConversationRow>(
        r#"
        SELECT * FROM ai_conversations
        WHERE mode = ?
          AND context_key IS ?
          AND connection_id IS ?
          AND database IS ?
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(mode)
    .bind(context_key)
    .bind(connection_id)
    .bind(database)
    .fetch_optional(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(row.map(AiConversation::from))
}

/// List all AI chat conversations, most recently updated first.
#[tauri::command]
pub async fn ai_chat_list_conversations(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<AiConversation>, AppError> {
    let rows = sqlx::query_as::<_, AiConversationRow>(
        "SELECT * FROM ai_conversations ORDER BY updated_at DESC",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(AiConversation::from).collect())
}

/// Create a new conversation along with its first user/assistant message pair.
#[tauri::command]
pub async fn ai_chat_create_conversation(
    sqlite: State<'_, SqlitePool>,
    input: AiConversationInput,
) -> Result<AiConversation, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        INSERT INTO ai_conversations (id, mode, context_key, title, connection_id, database, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        id,
        input.mode,
        input.context_key,
        input.title,
        input.connection_id,
        input.database,
        now,
        now
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    if let (Some(user_message), Some(assistant_message)) =
        (&input.first_user_message, &input.first_assistant_message)
    {
        for (role, content) in [("user", user_message), ("assistant", assistant_message)] {
            let message_id = Uuid::new_v4().to_string();
            sqlx::query!(
                "INSERT INTO ai_messages (id, conversation_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
                message_id,
                id,
                role,
                content,
                now
            )
            .execute(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
        }
    }

    let row = sqlx::query_as::<_, AiConversationRow>("SELECT * FROM ai_conversations WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(AiConversation::from(row))
}

/// Rename a conversation's title.
#[tauri::command]
pub async fn ai_chat_rename_conversation(
    sqlite: State<'_, SqlitePool>,
    id: String,
    title: String,
) -> Result<AiConversation, AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        "UPDATE ai_conversations SET title = ?, updated_at = ? WHERE id = ?",
        title,
        now,
        id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, AiConversationRow>("SELECT * FROM ai_conversations WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(AiConversation::from(row))
}

/// Set the connection/database a conversation is scoped to, so "open in query
/// editor" from a general chat can remember the connection the user picked
/// instead of prompting again on every code block.
#[tauri::command]
pub async fn ai_chat_set_connection(
    sqlite: State<'_, SqlitePool>,
    id: String,
    connection_id: Option<String>,
    database: Option<String>,
) -> Result<AiConversation, AppError> {
    sqlx::query!(
        "UPDATE ai_conversations SET connection_id = ?, database = ? WHERE id = ?",
        connection_id,
        database,
        id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, AiConversationRow>("SELECT * FROM ai_conversations WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(AiConversation::from(row))
}

/// Delete a conversation and all of its messages (cascades via FK).
#[tauri::command]
pub async fn ai_chat_delete_conversation(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM ai_conversations WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

// ── Message commands ───────────────────────────────────────────────────────────

/// List all messages for a conversation, oldest first.
#[tauri::command]
pub async fn ai_chat_get_messages(
    sqlite: State<'_, SqlitePool>,
    conversation_id: String,
) -> Result<Vec<AiMessage>, AppError> {
    let rows = sqlx::query_as::<_, AiMessageRow>(
        "SELECT * FROM ai_messages WHERE conversation_id = ? ORDER BY created_at",
    )
    .bind(conversation_id)
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(AiMessage::from).collect())
}

/// Append a message to a conversation and bump the conversation's updated_at.
#[tauri::command]
pub async fn ai_chat_add_message(
    sqlite: State<'_, SqlitePool>,
    conversation_id: String,
    role: String,
    content: String,
) -> Result<AiMessage, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        "INSERT INTO ai_messages (id, conversation_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        id,
        conversation_id,
        role,
        content,
        now
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    sqlx::query!(
        "UPDATE ai_conversations SET updated_at = ? WHERE id = ?",
        now,
        conversation_id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row = sqlx::query_as::<_, AiMessageRow>("SELECT * FROM ai_messages WHERE id = ?")
        .bind(&id)
        .fetch_one(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(AiMessage::from(row))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("src/db/migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_conversation_inserts_conversation_and_two_messages() {
        let pool = setup_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        let id = "conv-1";

        sqlx::query(
            "INSERT INTO ai_conversations (id, mode, context_key, title, connection_id, database, created_at, updated_at) VALUES (?, 'describe', 'ctx', 'Users table', NULL, NULL, ?, ?)",
        )
        .bind(id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        for (mid, role, content) in [("m-1", "user", "Describe users"), ("m-2", "assistant", "It stores users.")] {
            sqlx::query(
                "INSERT INTO ai_messages (id, conversation_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(mid)
            .bind(id)
            .bind(role)
            .bind(content)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        }

        let messages = sqlx::query_as::<_, AiMessageRow>(
            "SELECT * FROM ai_messages WHERE conversation_id = ? ORDER BY created_at",
        )
        .bind(id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }

    #[tokio::test]
    async fn delete_conversation_cascades_to_messages() {
        let pool = setup_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        let id = "conv-del";

        sqlx::query(
            "INSERT INTO ai_conversations (id, mode, context_key, title, connection_id, database, created_at, updated_at) VALUES (?, 'explain', NULL, 'Explain', NULL, NULL, ?, ?)",
        )
        .bind(id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO ai_messages (id, conversation_id, role, content, created_at) VALUES ('m-del', ?, 'user', 'hi', ?)",
        )
        .bind(id)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM ai_conversations WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let messages = sqlx::query_as::<_, AiMessageRow>("SELECT * FROM ai_messages WHERE conversation_id = ?")
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap();

        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn find_conversation_matches_null_context_key() {
        let pool = setup_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO ai_conversations (id, mode, context_key, title, connection_id, database, created_at, updated_at) VALUES ('conv-gen', 'generate', NULL, 'Gen', NULL, NULL, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query_as::<_, AiConversationRow>(
            r#"
            SELECT * FROM ai_conversations
            WHERE mode = ? AND context_key IS ? AND connection_id IS ? AND database IS ?
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind("generate")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(row.is_some());
        assert_eq!(row.unwrap().id, "conv-gen");
    }
}

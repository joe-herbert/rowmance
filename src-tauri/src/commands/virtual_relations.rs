/// Tauri commands for managing virtual column connections.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;

// ── Polymorphic virtual relations ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PolymorphicMapping {
    pub id: String,
    #[serde(rename = "typeValue")]
    pub type_value: String,
    pub to: ColumnRef,
}

#[derive(Serialize, Debug, Clone)]
pub struct PolymorphicVirtualRelation {
    pub id: String,
    pub label: Option<String>,
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub database: String,
    pub table: String,
    #[serde(rename = "typeColumn")]
    pub type_column: String,
    #[serde(rename = "valueColumn")]
    pub value_column: String,
    pub mappings: Vec<PolymorphicMapping>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PolymorphicMappingInput {
    #[serde(rename = "typeValue")]
    pub type_value: String,
    pub to: ColumnRef,
}

#[derive(Deserialize, Debug)]
pub struct PolymorphicVirtualRelationInput {
    pub label: Option<String>,
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub database: String,
    pub table: String,
    #[serde(rename = "typeColumn")]
    pub type_column: String,
    #[serde(rename = "valueColumn")]
    pub value_column: String,
    pub mappings: Vec<PolymorphicMappingInput>,
}

#[derive(sqlx::FromRow)]
struct PolymorphicVrRow {
    id: String,
    label: Option<String>,
    connection_id: String,
    database: String,
    table_name: String,
    type_column: String,
    value_column: String,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct PolymorphicMappingRow {
    id: String,
    relation_id: String,
    type_value: String,
    to_connection_id: String,
    to_database: String,
    to_table: String,
    to_column: String,
}

async fn build_polymorphic_relation(
    sqlite: &SqlitePool,
    row: PolymorphicVrRow,
) -> Result<PolymorphicVirtualRelation, AppError> {
    let mapping_rows = sqlx::query_as::<_, PolymorphicMappingRow>(
        "SELECT * FROM polymorphic_vr_mappings WHERE relation_id = ? ORDER BY rowid",
    )
    .bind(&row.id)
    .fetch_all(sqlite)
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let mappings = mapping_rows
        .into_iter()
        .map(|m| PolymorphicMapping {
            id: m.id,
            type_value: m.type_value,
            to: ColumnRef {
                connection_id: m.to_connection_id,
                database: m.to_database,
                table: m.to_table,
                column: m.to_column,
            },
        })
        .collect();

    Ok(PolymorphicVirtualRelation {
        id: row.id,
        label: row.label,
        connection_id: row.connection_id,
        database: row.database,
        table: row.table_name,
        type_column: row.type_column,
        value_column: row.value_column,
        mappings,
    })
}

#[tauri::command]
pub async fn polymorphic_vr_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<PolymorphicVirtualRelation>, AppError> {
    let rows = sqlx::query_as::<_, PolymorphicVrRow>(
        "SELECT id, label, connection_id, database, table_name, type_column, value_column \
         FROM polymorphic_virtual_relations ORDER BY created_at",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        result.push(build_polymorphic_relation(sqlite.inner(), row).await?);
    }
    Ok(result)
}

#[tauri::command]
pub async fn polymorphic_vr_create(
    sqlite: State<'_, SqlitePool>,
    input: PolymorphicVirtualRelationInput,
) -> Result<PolymorphicVirtualRelation, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        INSERT INTO polymorphic_virtual_relations
            (id, label, connection_id, database, table_name, type_column, value_column, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        id,
        input.label,
        input.connection_id,
        input.database,
        input.table,
        input.type_column,
        input.value_column,
        now,
        now,
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    for mapping in &input.mappings {
        let mapping_id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"
            INSERT INTO polymorphic_vr_mappings
                (id, relation_id, type_value, to_connection_id, to_database, to_table, to_column)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            mapping_id,
            id,
            mapping.type_value,
            mapping.to.connection_id,
            mapping.to.database,
            mapping.to.table,
            mapping.to.column,
        )
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }

    let row = sqlx::query_as::<_, PolymorphicVrRow>(
        "SELECT id, label, connection_id, database, table_name, type_column, value_column \
         FROM polymorphic_virtual_relations WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    build_polymorphic_relation(sqlite.inner(), row).await
}

#[tauri::command]
pub async fn polymorphic_vr_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: PolymorphicVirtualRelationInput,
) -> Result<PolymorphicVirtualRelation, AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        UPDATE polymorphic_virtual_relations
        SET label = ?, connection_id = ?, database = ?, table_name = ?,
            type_column = ?, value_column = ?, updated_at = ?
        WHERE id = ?
        "#,
        input.label,
        input.connection_id,
        input.database,
        input.table,
        input.type_column,
        input.value_column,
        now,
        id,
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    sqlx::query!(
        "DELETE FROM polymorphic_vr_mappings WHERE relation_id = ?",
        id
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    for mapping in &input.mappings {
        let mapping_id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"
            INSERT INTO polymorphic_vr_mappings
                (id, relation_id, type_value, to_connection_id, to_database, to_table, to_column)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            mapping_id,
            id,
            mapping.type_value,
            mapping.to.connection_id,
            mapping.to.database,
            mapping.to.table,
            mapping.to.column,
        )
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    }

    let row = sqlx::query_as::<_, PolymorphicVrRow>(
        "SELECT id, label, connection_id, database, table_name, type_column, value_column \
         FROM polymorphic_virtual_relations WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    build_polymorphic_relation(sqlite.inner(), row).await
}

#[tauri::command]
pub async fn polymorphic_vr_delete(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM polymorphic_virtual_relations WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ColumnRef {
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub database: String,
    pub table: String,
    pub column: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct VirtualRelation {
    pub id: String,
    pub label: Option<String>,
    pub from: ColumnRef,
    pub to: ColumnRef,
}

#[derive(Deserialize, Debug)]
pub struct VirtualRelationInput {
    pub label: Option<String>,
    pub from: ColumnRef,
    pub to: ColumnRef,
}

// ── SQLite row type ───────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct VirtualRelationRow {
    id: String,
    label: Option<String>,
    from_connection_id: String,
    from_database: String,
    from_table: String,
    from_column: String,
    to_connection_id: String,
    to_database: String,
    to_table: String,
    to_column: String,
}

impl From<VirtualRelationRow> for VirtualRelation {
    fn from(r: VirtualRelationRow) -> Self {
        Self {
            id: r.id,
            label: r.label,
            from: ColumnRef {
                connection_id: r.from_connection_id,
                database: r.from_database,
                table: r.from_table,
                column: r.from_column,
            },
            to: ColumnRef {
                connection_id: r.to_connection_id,
                database: r.to_database,
                table: r.to_table,
                column: r.to_column,
            },
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn virtual_relations_list(
    sqlite: State<'_, SqlitePool>,
) -> Result<Vec<VirtualRelation>, AppError> {
    let rows = sqlx::query_as::<_, VirtualRelationRow>(
        "SELECT * FROM virtual_relations ORDER BY created_at",
    )
    .fetch_all(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(rows.into_iter().map(VirtualRelation::from).collect())
}

#[tauri::command]
pub async fn virtual_relations_create(
    sqlite: State<'_, SqlitePool>,
    input: VirtualRelationInput,
) -> Result<VirtualRelation, AppError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM virtual_relations \
         WHERE from_connection_id = ? AND from_database = ? AND from_table = ? AND from_column = ? \
           AND to_connection_id = ?   AND to_database = ?   AND to_table = ?   AND to_column = ?",
    )
    .bind(&input.from.connection_id)
    .bind(&input.from.database)
    .bind(&input.from.table)
    .bind(&input.from.column)
    .bind(&input.to.connection_id)
    .bind(&input.to.database)
    .bind(&input.to.table)
    .bind(&input.to.column)
    .fetch_one(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    if count > 0 {
        return Err(AppError::new(
            "DUPLICATE_RELATION",
            "A virtual relation between these two columns already exists",
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        INSERT INTO virtual_relations (
            id, label,
            from_connection_id, from_database, from_table, from_column,
            to_connection_id, to_database, to_table, to_column,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        id,
        input.label,
        input.from.connection_id,
        input.from.database,
        input.from.table,
        input.from.column,
        input.to.connection_id,
        input.to.database,
        input.to.table,
        input.to.column,
        now,
        now,
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, VirtualRelationRow>("SELECT * FROM virtual_relations WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(VirtualRelation::from(row))
}

#[tauri::command]
pub async fn virtual_relations_update(
    sqlite: State<'_, SqlitePool>,
    id: String,
    input: VirtualRelationInput,
) -> Result<VirtualRelation, AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        UPDATE virtual_relations
        SET label = ?,
            from_connection_id = ?, from_database = ?, from_table = ?, from_column = ?,
            to_connection_id = ?, to_database = ?, to_table = ?, to_column = ?,
            updated_at = ?
        WHERE id = ?
        "#,
        input.label,
        input.from.connection_id,
        input.from.database,
        input.from.table,
        input.from.column,
        input.to.connection_id,
        input.to.database,
        input.to.table,
        input.to.column,
        now,
        id,
    )
    .execute(sqlite.inner())
    .await
    .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    let row =
        sqlx::query_as::<_, VirtualRelationRow>("SELECT * FROM virtual_relations WHERE id = ?")
            .bind(&id)
            .fetch_one(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;

    Ok(VirtualRelation::from(row))
}

#[tauri::command]
pub async fn virtual_relations_delete(
    sqlite: State<'_, SqlitePool>,
    id: String,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM virtual_relations WHERE id = ?", id)
        .execute(sqlite.inner())
        .await
        .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

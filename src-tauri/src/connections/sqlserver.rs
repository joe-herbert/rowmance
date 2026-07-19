/// SQL Server-specific schema introspection queries using tiberius.
use futures::TryStreamExt;
use tiberius::{Client, QueryItem};
use tokio_util::compat::Compat;

use crate::connections::types::{ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo};
use crate::error::RowmanceError;

pub type MssqlConn = Client<Compat<tokio::net::TcpStream>>;

/// Execute a statement with no parameters via the text batch protocol.
/// Use this for DDL and transaction control (BEGIN/COMMIT/ROLLBACK).
/// `execute()` uses sp_executesql which SQL Server rejects for these
/// statements when the transaction count changes within the batch.
pub async fn exec_simple(conn: &mut MssqlConn, sql: &str) -> Result<(), RowmanceError> {
    use futures::TryStreamExt;
    let mut stream = conn
        .simple_query(sql)
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    while stream
        .try_next()
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
        .is_some()
    {}
    Ok(())
}

/// Helper: get a string value from a tiberius Row by column index using typed access.
fn get_str(row: &tiberius::Row, idx: usize) -> Option<String> {
    // Try as &str first (NVarChar, VarChar), then owned String
    if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
        return Some(v.to_owned());
    }
    None
}

fn get_i32(row: &tiberius::Row, idx: usize) -> Option<i32> {
    row.try_get::<i32, _>(idx).ok().flatten()
}

fn get_i64(row: &tiberius::Row, idx: usize) -> Option<i64> {
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<i32, _>(idx) {
        return v.map(|n| n as i64);
    }
    None
}

fn get_bit(row: &tiberius::Row, idx: usize) -> Option<bool> {
    row.try_get::<bool, _>(idx).ok().flatten()
}

/// Returns "[db_name]." as a SQL identifier prefix for cross-database queries,
/// or an empty string when instance_db is None.
fn db_prefix(instance_db: Option<&str>) -> String {
    match instance_db {
        Some(db) => format!("[{}].", db.replace(']', "]]")),
        None => String::new(),
    }
}

/// Collect all rows from a query stream into a Vec.
async fn collect_rows(
    conn: &mut MssqlConn,
    sql: &str,
    params: &[&dyn tiberius::ToSql],
) -> Result<Vec<tiberius::Row>, RowmanceError> {
    let mut stream = conn
        .query(sql, params)
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut rows = Vec::new();
    while let Some(item) = stream
        .try_next()
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    {
        if let QueryItem::Row(row) = item {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Collect rows for a simple query (no parameters).
async fn collect_rows_simple(
    conn: &mut MssqlConn,
    sql: &str,
) -> Result<Vec<tiberius::Row>, RowmanceError> {
    let mut stream = conn
        .simple_query(sql)
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut rows = Vec::new();
    while let Some(item) = stream
        .try_next()
        .await
        .map_err(|e| RowmanceError::Pool(e.to_string()))?
    {
        if let QueryItem::Row(row) = item {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// List all user schemas in a SQL Server database.
/// Uses three-part name [instance_db].INFORMATION_SCHEMA.SCHEMATA for cross-database access.
pub async fn list_schemas_in_database(
    conn: &mut MssqlConn,
    instance_db: &str,
) -> Result<Vec<String>, RowmanceError> {
    let db_esc = instance_db.replace(']', "]]");
    let sql = format!(
        "
        SELECT SCHEMA_NAME
        FROM [{db_esc}].INFORMATION_SCHEMA.SCHEMATA
        WHERE SCHEMA_NAME NOT IN (
            'sys', 'INFORMATION_SCHEMA', 'guest', 'db_owner',
            'db_accessadmin', 'db_securityadmin', 'db_ddladmin',
            'db_backupoperator', 'db_datareader', 'db_datawriter',
            'db_denydatareader', 'db_denydatawriter'
        )
        ORDER BY SCHEMA_NAME
    "
    );
    let rows = collect_rows_simple(conn, &sql).await?;
    Ok(rows.iter().filter_map(|r| get_str(r, 0)).collect())
}

/// List all user databases on the SQL Server instance.
pub async fn list_instance_databases(conn: &mut MssqlConn) -> Result<Vec<String>, RowmanceError> {
    let sql = "
        SELECT name
        FROM sys.databases
        WHERE name NOT IN ('master', 'tempdb', 'model', 'msdb')
          AND state_desc = 'ONLINE'
        ORDER BY name
    ";
    let rows = collect_rows_simple(conn, sql).await?;
    Ok(rows.iter().filter_map(|r| get_str(r, 0)).collect())
}

/// List all tables and views in the given schema.
pub async fn list_tables(
    conn: &mut MssqlConn,
    schema: &str,
    instance_db: Option<&str>,
) -> Result<Vec<TableInfo>, RowmanceError> {
    let prefix = db_prefix(instance_db);
    let sql = format!(
        "SELECT TABLE_NAME, TABLE_TYPE FROM {prefix}INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = @P1 ORDER BY TABLE_NAME",
        prefix = prefix
    );
    let rows = collect_rows(conn, &sql, &[&schema]).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let name = get_str(r, 0).unwrap_or_default();
            let ttype = get_str(r, 1).unwrap_or_default();
            TableInfo {
                name,
                table_type: if ttype == "VIEW" {
                    "view".to_string()
                } else {
                    "table".to_string()
                },
                row_count: None, // filled in by background count
            }
        })
        .collect())
}

/// List all columns for a table or view.
pub async fn list_columns(
    conn: &mut MssqlConn,
    schema: &str,
    table: &str,
    instance_db: Option<&str>,
) -> Result<Vec<ColumnInfo>, RowmanceError> {
    let prefix = db_prefix(instance_db);
    let sql = format!("
        SELECT
            c.COLUMN_NAME,
            c.DATA_TYPE +
                CASE
                    WHEN c.CHARACTER_MAXIMUM_LENGTH IS NOT NULL
                         AND c.DATA_TYPE NOT IN ('text','ntext','image','xml')
                    THEN '(' + CAST(c.CHARACTER_MAXIMUM_LENGTH AS NVARCHAR(20)) + ')'
                    WHEN c.NUMERIC_PRECISION IS NOT NULL AND c.NUMERIC_SCALE IS NOT NULL
                         AND c.DATA_TYPE IN ('decimal','numeric')
                    THEN '(' + CAST(c.NUMERIC_PRECISION AS NVARCHAR(10)) + ',' + CAST(c.NUMERIC_SCALE AS NVARCHAR(10)) + ')'
                    ELSE ''
                END AS full_type,
            c.IS_NULLABLE,
            c.COLUMN_DEFAULT,
            COLUMNPROPERTY(OBJECT_ID(N'{prefix}[' + c.TABLE_SCHEMA + N'].[' + c.TABLE_NAME + N']'), c.COLUMN_NAME, 'IsIdentity') AS is_identity,
            CASE WHEN kcu.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS is_pk,
            CASE WHEN fk.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS is_fk
        FROM {prefix}INFORMATION_SCHEMA.COLUMNS c
        LEFT JOIN (
            SELECT kcu2.COLUMN_NAME
            FROM {prefix}INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc
            JOIN {prefix}INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu2
                ON tc.CONSTRAINT_NAME = kcu2.CONSTRAINT_NAME
                AND tc.TABLE_SCHEMA = kcu2.TABLE_SCHEMA
                AND tc.TABLE_NAME = kcu2.TABLE_NAME
            WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY'
                AND tc.TABLE_SCHEMA = @P1
                AND tc.TABLE_NAME = @P2
        ) kcu ON kcu.COLUMN_NAME = c.COLUMN_NAME
        LEFT JOIN (
            SELECT DISTINCT cu.COLUMN_NAME
            FROM {prefix}INFORMATION_SCHEMA.CONSTRAINT_COLUMN_USAGE cu
            JOIN {prefix}INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc2
                ON tc2.CONSTRAINT_NAME = cu.CONSTRAINT_NAME
                AND tc2.CONSTRAINT_SCHEMA = cu.CONSTRAINT_SCHEMA
            WHERE tc2.CONSTRAINT_TYPE = 'FOREIGN KEY'
                AND cu.TABLE_SCHEMA = @P1
                AND cu.TABLE_NAME = @P2
        ) fk ON fk.COLUMN_NAME = c.COLUMN_NAME
        WHERE c.TABLE_SCHEMA = @P1 AND c.TABLE_NAME = @P2
        ORDER BY c.ORDINAL_POSITION
    ", prefix = prefix);
    let rows = collect_rows(conn, &sql, &[&schema, &table]).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let name = get_str(r, 0).unwrap_or_default();
            let data_type = get_str(r, 1).unwrap_or_default();
            let nullable = get_str(r, 2).map(|s| s == "YES").unwrap_or(true);
            let default_value = get_str(r, 3);
            let is_auto = get_i32(r, 4).unwrap_or(0) == 1;
            let is_pk = get_i32(r, 5).unwrap_or(0) == 1;
            let is_fk = get_i32(r, 6).unwrap_or(0) == 1;
            ColumnInfo {
                name,
                data_type,
                nullable,
                default_value,
                is_primary_key: is_pk,
                is_auto_increment: is_auto,
                is_foreign_key: is_fk,
                comment: None,
            }
        })
        .collect())
}

/// List all columns for every table and view in the schema.
pub async fn list_all_columns(
    conn: &mut MssqlConn,
    schema: &str,
    instance_db: Option<&str>,
) -> Result<Vec<(String, ColumnInfo)>, RowmanceError> {
    let prefix = db_prefix(instance_db);
    let sql = format!("
        SELECT
            c.TABLE_NAME,
            c.COLUMN_NAME,
            c.DATA_TYPE +
                CASE
                    WHEN c.CHARACTER_MAXIMUM_LENGTH IS NOT NULL
                         AND c.DATA_TYPE NOT IN ('text','ntext','image','xml')
                    THEN '(' + CAST(c.CHARACTER_MAXIMUM_LENGTH AS NVARCHAR(20)) + ')'
                    WHEN c.NUMERIC_PRECISION IS NOT NULL AND c.NUMERIC_SCALE IS NOT NULL
                         AND c.DATA_TYPE IN ('decimal','numeric')
                    THEN '(' + CAST(c.NUMERIC_PRECISION AS NVARCHAR(10)) + ',' + CAST(c.NUMERIC_SCALE AS NVARCHAR(10)) + ')'
                    ELSE ''
                END AS full_type,
            c.IS_NULLABLE,
            c.COLUMN_DEFAULT,
            COLUMNPROPERTY(OBJECT_ID(N'{prefix}[' + c.TABLE_SCHEMA + N'].[' + c.TABLE_NAME + N']'), c.COLUMN_NAME, 'IsIdentity') AS is_identity,
            CASE WHEN pk.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS is_pk,
            CASE WHEN fk.COLUMN_NAME IS NOT NULL THEN 1 ELSE 0 END AS is_fk
        FROM {prefix}INFORMATION_SCHEMA.COLUMNS c
        LEFT JOIN (
            SELECT kcu.TABLE_NAME, kcu.COLUMN_NAME
            FROM {prefix}INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc
            JOIN {prefix}INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu
                ON tc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME
                AND tc.TABLE_SCHEMA = kcu.TABLE_SCHEMA
                AND tc.TABLE_NAME = kcu.TABLE_NAME
            WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY'
                AND tc.TABLE_SCHEMA = @P1
        ) pk ON pk.TABLE_NAME = c.TABLE_NAME AND pk.COLUMN_NAME = c.COLUMN_NAME
        LEFT JOIN (
            SELECT DISTINCT cu.TABLE_NAME, cu.COLUMN_NAME
            FROM {prefix}INFORMATION_SCHEMA.CONSTRAINT_COLUMN_USAGE cu
            JOIN {prefix}INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc2
                ON tc2.CONSTRAINT_NAME = cu.CONSTRAINT_NAME
                AND tc2.CONSTRAINT_SCHEMA = cu.CONSTRAINT_SCHEMA
            WHERE tc2.CONSTRAINT_TYPE = 'FOREIGN KEY'
                AND cu.TABLE_SCHEMA = @P1
        ) fk ON fk.TABLE_NAME = c.TABLE_NAME AND fk.COLUMN_NAME = c.COLUMN_NAME
        WHERE c.TABLE_SCHEMA = @P1
        ORDER BY c.TABLE_NAME, c.ORDINAL_POSITION
    ", prefix = prefix);
    let rows = collect_rows(conn, &sql, &[&schema]).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let table_name = get_str(r, 0).unwrap_or_default();
            let name = get_str(r, 1).unwrap_or_default();
            let data_type = get_str(r, 2).unwrap_or_default();
            let nullable = get_str(r, 3).map(|s| s == "YES").unwrap_or(true);
            let default_value = get_str(r, 4);
            let is_auto = get_i32(r, 5).unwrap_or(0) == 1;
            let is_pk = get_i32(r, 6).unwrap_or(0) == 1;
            let is_fk = get_i32(r, 7).unwrap_or(0) == 1;
            (
                table_name,
                ColumnInfo {
                    name,
                    data_type,
                    nullable,
                    default_value,
                    is_primary_key: is_pk,
                    is_auto_increment: is_auto,
                    is_foreign_key: is_fk,
                    comment: None,
                },
            )
        })
        .collect())
}

/// List indexes for a table.
pub async fn list_indexes(
    conn: &mut MssqlConn,
    schema: &str,
    table: &str,
    instance_db: Option<&str>,
) -> Result<Vec<IndexInfo>, RowmanceError> {
    let prefix = db_prefix(instance_db);
    let sql = format!(
        "
        SELECT
            i.name AS index_name,
            c.name AS column_name,
            i.is_unique,
            i.type_desc
        FROM {prefix}sys.indexes i
        JOIN {prefix}sys.index_columns ic
            ON i.object_id = ic.object_id AND i.index_id = ic.index_id
        JOIN {prefix}sys.columns c
            ON ic.object_id = c.object_id AND ic.column_id = c.column_id
        WHERE i.object_id = OBJECT_ID(N'{prefix}[' + @P1 + N'].[' + @P2 + N']')
            AND i.name IS NOT NULL
            AND i.is_hypothetical = 0
        ORDER BY i.name, ic.key_ordinal
    ",
        prefix = prefix
    );
    let rows = collect_rows(conn, &sql, &[&schema, &table]).await?;

    let mut map: std::collections::BTreeMap<String, IndexInfo> = std::collections::BTreeMap::new();
    for r in &rows {
        let name = get_str(r, 0).unwrap_or_default();
        let col = get_str(r, 1).unwrap_or_default();
        let unique = get_bit(r, 2).unwrap_or(false);
        let type_desc = get_str(r, 3).unwrap_or_else(|| "NONCLUSTERED".to_string());
        let entry = map.entry(name.clone()).or_insert_with(|| IndexInfo {
            name: name.clone(),
            columns: vec![],
            unique,
            index_type: type_desc,
        });
        entry.columns.push(col);
    }
    Ok(map.into_values().collect())
}

/// List foreign keys for a table.
pub async fn list_foreign_keys(
    conn: &mut MssqlConn,
    schema: &str,
    table: &str,
    instance_db: Option<&str>,
) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
    let prefix = db_prefix(instance_db);
    let sql = format!(
        "
        SELECT
            fk.name AS constraint_name,
            c_parent.name AS column_name,
            OBJECT_NAME(fk.referenced_object_id) AS referenced_table,
            c_ref.name AS referenced_column,
            fk.delete_referential_action_desc AS on_delete,
            fk.update_referential_action_desc AS on_update
        FROM {prefix}sys.foreign_keys fk
        JOIN {prefix}sys.foreign_key_columns fkc
            ON fk.object_id = fkc.constraint_object_id
        JOIN {prefix}sys.columns c_parent
            ON fkc.parent_object_id = c_parent.object_id
            AND fkc.parent_column_id = c_parent.column_id
        JOIN {prefix}sys.columns c_ref
            ON fkc.referenced_object_id = c_ref.object_id
            AND fkc.referenced_column_id = c_ref.column_id
        WHERE fk.parent_object_id = OBJECT_ID(N'{prefix}[' + @P1 + N'].[' + @P2 + N']')
        ORDER BY fk.name, fkc.constraint_column_id
    ",
        prefix = prefix
    );
    let rows = collect_rows(conn, &sql, &[&schema, &table]).await?;

    let mut map: std::collections::BTreeMap<String, ForeignKeyInfo> =
        std::collections::BTreeMap::new();
    for r in &rows {
        let name = get_str(r, 0).unwrap_or_default();
        let col = get_str(r, 1).unwrap_or_default();
        let ref_table = get_str(r, 2).unwrap_or_default();
        let ref_col = get_str(r, 3).unwrap_or_default();
        let on_delete = get_str(r, 4).unwrap_or_else(|| "NO_ACTION".to_string());
        let on_update = get_str(r, 5).unwrap_or_else(|| "NO_ACTION".to_string());
        let entry = map.entry(name.clone()).or_insert_with(|| ForeignKeyInfo {
            constraint_name: name.clone(),
            columns: vec![],
            referenced_table: ref_table,
            referenced_columns: vec![],
            on_delete: on_delete.replace('_', " "),
            on_update: on_update.replace('_', " "),
        });
        entry.columns.push(col);
        entry.referenced_columns.push(ref_col);
    }
    Ok(map.into_values().collect())
}

/// Count rows in a table using sys.partitions (fast, no full scan).
pub async fn count_table(
    conn: &mut MssqlConn,
    schema: &str,
    table: &str,
    instance_db: Option<&str>,
) -> Result<i64, RowmanceError> {
    let prefix = db_prefix(instance_db);
    let sql = format!(
        "
        SELECT SUM(p.rows)
        FROM {prefix}sys.indexes i
        JOIN {prefix}sys.partitions p
            ON i.object_id = p.object_id AND i.index_id = p.index_id
        WHERE i.object_id = OBJECT_ID(N'{prefix}[' + @P1 + N'].[' + @P2 + N']')
            AND i.index_id <= 1
    ",
        prefix = prefix
    );
    let rows = collect_rows(conn, &sql, &[&schema, &table]).await?;
    if let Some(r) = rows.first() {
        return Ok(get_i64(r, 0).unwrap_or(0));
    }
    Ok(0)
}

/// Get the DDL for a table or view.
/// For views, uses OBJECT_DEFINITION(); for tables, constructs from sys.columns.
pub async fn get_ddl(
    conn: &mut MssqlConn,
    schema: &str,
    table: &str,
    instance_db: Option<&str>,
) -> Result<String, RowmanceError> {
    let prefix = db_prefix(instance_db);
    // Check if it's a view first.
    let view_sql = format!(
        "SELECT OBJECT_DEFINITION(OBJECT_ID(N'{prefix}[' + @P1 + N'].[' + @P2 + N']'))",
        prefix = prefix
    );
    let view_rows = collect_rows(conn, &view_sql, &[&schema, &table]).await?;
    if let Some(r) = view_rows.first() {
        if let Some(def) = get_str(r, 0) {
            if !def.is_empty() {
                return Ok(def);
            }
        }
    }

    // It's a table — construct DDL from columns.
    let col_sql = format!(
        "
        SELECT
            c.name,
            tp.name AS type_name,
            c.max_length,
            c.precision,
            c.scale,
            c.is_nullable,
            c.is_identity,
            OBJECT_DEFINITION(c.default_object_id) AS default_def,
            c.collation_name
        FROM {prefix}sys.columns c
        JOIN {prefix}sys.types tp ON c.user_type_id = tp.user_type_id
        WHERE c.object_id = OBJECT_ID(N'{prefix}[' + @P1 + N'].[' + @P2 + N']')
        ORDER BY c.column_id
    ",
        prefix = prefix
    );
    let col_rows = collect_rows(conn, &col_sql, &[&schema, &table]).await?;

    // Primary keys
    let pk_sql = format!(
        "
        SELECT c.name
        FROM {prefix}sys.indexes i
        JOIN {prefix}sys.index_columns ic
            ON i.object_id = ic.object_id AND i.index_id = ic.index_id
        JOIN {prefix}sys.columns c
            ON ic.object_id = c.object_id AND ic.column_id = c.column_id
        WHERE i.object_id = OBJECT_ID(N'{prefix}[' + @P1 + N'].[' + @P2 + N']')
            AND i.is_primary_key = 1
        ORDER BY ic.key_ordinal
    ",
        prefix = prefix
    );
    let pk_rows = collect_rows(conn, &pk_sql, &[&schema, &table]).await?;
    let pk_cols: Vec<String> = pk_rows.iter().filter_map(|r| get_str(r, 0)).collect();

    let mut col_defs: Vec<String> = Vec::new();
    for r in &col_rows {
        let col_name = get_str(r, 0).unwrap_or_default();
        let type_name = get_str(r, 1).unwrap_or_default();
        let max_length: i16 = r.try_get::<i16, _>(2).ok().flatten().unwrap_or(0);
        let precision: u8 = r.try_get::<u8, _>(3).ok().flatten().unwrap_or(0);
        let scale: u8 = r.try_get::<u8, _>(4).ok().flatten().unwrap_or(0);
        let is_nullable = get_bit(r, 5).unwrap_or(true);
        let is_identity = get_bit(r, 6).unwrap_or(false);
        let default_def = get_str(r, 7);

        let type_str = match type_name.as_str() {
            "nvarchar" | "nchar" => {
                if max_length == -1 {
                    format!("{}(MAX)", type_name)
                } else {
                    format!("{}({})", type_name, max_length / 2)
                }
            }
            "varchar" | "char" | "varbinary" | "binary" => {
                if max_length == -1 {
                    format!("{}(MAX)", type_name)
                } else {
                    format!("{}({})", type_name, max_length)
                }
            }
            "decimal" | "numeric" => format!("{}({},{})", type_name, precision, scale),
            _ => type_name.clone(),
        };

        let mut def = format!("    [{}] {}", col_name, type_str);
        if is_identity {
            def.push_str(" IDENTITY(1,1)");
        }
        if let Some(d) = default_def {
            def.push_str(&format!(" DEFAULT {}", d));
        }
        if !is_nullable {
            def.push_str(" NOT NULL");
        } else {
            def.push_str(" NULL");
        }
        col_defs.push(def);
    }

    if !pk_cols.is_empty() {
        let pk_list = pk_cols
            .iter()
            .map(|c| format!("[{}]", c))
            .collect::<Vec<_>>()
            .join(", ");
        col_defs.push(format!(
            "    CONSTRAINT [PK_{}] PRIMARY KEY ({})",
            table, pk_list
        ));
    }

    Ok(format!(
        "CREATE TABLE [{}].[{}] (\n{}\n);",
        schema,
        table,
        col_defs.join(",\n")
    ))
}

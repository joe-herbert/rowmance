/// Oracle-specific schema introspection queries using the oracle crate (sync).
use crate::connections::types::{
    CheckConstraintInfo, ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo, TriggerInfo, ViewInfo,
};
use crate::error::RowmanceError;

/// List all user tables and views in the given schema (Oracle owner).
pub fn list_tables(
    conn: &oracle::Connection,
    owner: &str,
) -> Result<Vec<TableInfo>, RowmanceError> {
    let sql = "
        SELECT table_name, 'table' AS table_type
        FROM all_tables
        WHERE owner = :1
        UNION ALL
        SELECT view_name, 'view' AS table_type
        FROM all_views
        WHERE owner = :2
        ORDER BY 1
    ";
    let owner_upper = owner.to_uppercase();
    let mut stmt = conn
        .statement(sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper, &owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut tables = Vec::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let ttype: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        tables.push(TableInfo {
            name,
            table_type: ttype,
            row_count: None,
        });
    }
    Ok(tables)
}

/// List all columns for a table or view, annotated with PK/FK/identity flags.
pub fn list_columns(
    conn: &oracle::Connection,
    owner: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, RowmanceError> {
    let owner_upper = owner.to_uppercase();
    let table_upper = table.to_uppercase();

    let col_sql = "
        SELECT
            c.column_name,
            c.data_type || CASE
                WHEN c.data_type IN ('VARCHAR2','NVARCHAR2','CHAR','NCHAR')
                    THEN '(' || c.char_length || ')'
                WHEN c.data_type = 'NUMBER' AND c.data_precision IS NOT NULL
                    THEN '(' || c.data_precision || ',' || NVL(c.data_scale, 0) || ')'
                ELSE ''
            END AS full_type,
            CASE c.nullable WHEN 'Y' THEN 1 ELSE 0 END AS nullable,
            c.data_default,
            c.column_id
        FROM all_tab_columns c
        WHERE c.owner = :1 AND c.table_name = :2
        ORDER BY c.column_id
    ";
    let mut stmt = conn
        .statement(col_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper, &table_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut cols: Vec<(String, String, bool, Option<String>)> = Vec::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let dtype: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let nullable_i: i64 = row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let default: Option<String> = row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        cols.push((name, dtype, nullable_i == 1, default));
    }

    // Fetch PK columns
    let pk_sql = "
        SELECT acc.column_name
        FROM all_constraints ac
        JOIN all_cons_columns acc
            ON acc.owner = ac.owner AND acc.constraint_name = ac.constraint_name
        WHERE ac.owner = :1 AND ac.table_name = :2 AND ac.constraint_type = 'P'
        ORDER BY acc.position
    ";
    let mut pk_stmt = conn
        .statement(pk_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let pk_rows = pk_stmt
        .query(&[&owner_upper, &table_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let mut pk_set = std::collections::HashSet::new();
    for row_result in pk_rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        pk_set.insert(col);
    }

    // Fetch FK columns
    let fk_sql = "
        SELECT DISTINCT acc.column_name
        FROM all_constraints ac
        JOIN all_cons_columns acc
            ON acc.owner = ac.owner AND acc.constraint_name = ac.constraint_name
        WHERE ac.owner = :1 AND ac.table_name = :2 AND ac.constraint_type = 'R'
    ";
    let mut fk_stmt = conn
        .statement(fk_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let fk_rows = fk_stmt
        .query(&[&owner_upper, &table_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let mut fk_set = std::collections::HashSet::new();
    for row_result in fk_rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        fk_set.insert(col);
    }

    // Fetch identity columns
    let id_sql = "
        SELECT column_name
        FROM all_tab_identity_cols
        WHERE owner = :1 AND table_name = :2
    ";
    let mut id_stmt = conn
        .statement(id_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let id_rows = id_stmt
        .query(&[&owner_upper, &table_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let mut id_set = std::collections::HashSet::new();
    for row_result in id_rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        id_set.insert(col);
    }

    Ok(cols
        .into_iter()
        .map(|(name, dtype, nullable, default)| {
            let is_pk = pk_set.contains(&name);
            let is_fk = fk_set.contains(&name);
            let is_identity = id_set.contains(&name);
            ColumnInfo {
                name,
                data_type: dtype,
                nullable,
                default_value: default,
                is_primary_key: is_pk,
                is_auto_increment: is_identity,
                is_foreign_key: is_fk,
                comment: None,
            }
        })
        .collect())
}

/// List all columns for every table and view in the schema.
pub fn list_all_columns(
    conn: &oracle::Connection,
    owner: &str,
) -> Result<Vec<(String, ColumnInfo)>, RowmanceError> {
    let owner_upper = owner.to_uppercase();

    let col_sql = "
        SELECT
            c.table_name,
            c.column_name,
            c.data_type || CASE
                WHEN c.data_type IN ('VARCHAR2','NVARCHAR2','CHAR','NCHAR')
                    THEN '(' || c.char_length || ')'
                WHEN c.data_type = 'NUMBER' AND c.data_precision IS NOT NULL
                    THEN '(' || c.data_precision || ',' || NVL(c.data_scale, 0) || ')'
                ELSE ''
            END AS full_type,
            CASE c.nullable WHEN 'Y' THEN 1 ELSE 0 END AS nullable,
            c.data_default
        FROM all_tab_columns c
        WHERE c.owner = :1
        ORDER BY c.table_name, c.column_id
    ";
    let mut stmt = conn
        .statement(col_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut raw: Vec<(String, String, String, bool, Option<String>)> = Vec::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let tname: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let cname: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let dtype: String = row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let nullable_i: i64 = row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let default: Option<String> = row.get(4).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        raw.push((tname, cname, dtype, nullable_i == 1, default));
    }

    // Fetch all PK columns in one query
    let pk_sql = "
        SELECT acc.table_name, acc.column_name
        FROM all_constraints ac
        JOIN all_cons_columns acc
            ON acc.owner = ac.owner AND acc.constraint_name = ac.constraint_name
        WHERE ac.owner = :1 AND ac.constraint_type = 'P'
    ";
    let mut pk_stmt = conn
        .statement(pk_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let pk_rows = pk_stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let mut pk_set = std::collections::HashSet::new();
    for row_result in pk_rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let tname: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let cname: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        pk_set.insert((tname, cname));
    }

    // Fetch all FK columns in one query
    let fk_sql = "
        SELECT DISTINCT acc.table_name, acc.column_name
        FROM all_constraints ac
        JOIN all_cons_columns acc
            ON acc.owner = ac.owner AND acc.constraint_name = ac.constraint_name
        WHERE ac.owner = :1 AND ac.constraint_type = 'R'
    ";
    let mut fk_stmt = conn
        .statement(fk_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let fk_rows = fk_stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let mut fk_set = std::collections::HashSet::new();
    for row_result in fk_rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let tname: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let cname: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        fk_set.insert((tname, cname));
    }

    // Fetch identity columns
    let id_sql = "
        SELECT table_name, column_name
        FROM all_tab_identity_cols
        WHERE owner = :1
    ";
    let mut id_stmt = conn
        .statement(id_sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let id_rows = id_stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let mut id_set = std::collections::HashSet::new();
    for row_result in id_rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let tname: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let cname: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        id_set.insert((tname, cname));
    }

    Ok(raw
        .into_iter()
        .map(|(tname, cname, dtype, nullable, default)| {
            let is_pk = pk_set.contains(&(tname.clone(), cname.clone()));
            let is_fk = fk_set.contains(&(tname.clone(), cname.clone()));
            let is_identity = id_set.contains(&(tname.clone(), cname.clone()));
            (
                tname,
                ColumnInfo {
                    name: cname,
                    data_type: dtype,
                    nullable,
                    default_value: default,
                    is_primary_key: is_pk,
                    is_auto_increment: is_identity,
                    is_foreign_key: is_fk,
                    comment: None,
                },
            )
        })
        .collect())
}

/// List indexes for a table.
pub fn list_indexes(
    conn: &oracle::Connection,
    owner: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, RowmanceError> {
    let owner_upper = owner.to_uppercase();
    let table_upper = table.to_uppercase();

    let sql = "
        SELECT
            i.index_name,
            ic.column_name,
            CASE i.uniqueness WHEN 'UNIQUE' THEN 1 ELSE 0 END AS is_unique,
            i.index_type
        FROM all_indexes i
        JOIN all_ind_columns ic
            ON ic.index_owner = i.owner AND ic.index_name = i.index_name
        WHERE i.owner = :1 AND i.table_name = :2
        ORDER BY i.index_name, ic.column_position
    ";
    let mut stmt = conn
        .statement(sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper, &table_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut map: std::collections::BTreeMap<String, IndexInfo> = std::collections::BTreeMap::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let unique_i: i64 = row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let itype: String = row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let entry = map.entry(name.clone()).or_insert_with(|| IndexInfo {
            name: name.clone(),
            columns: vec![],
            unique: unique_i == 1,
            index_type: itype,
        });
        entry.columns.push(col);
    }
    Ok(map.into_values().collect())
}

/// List foreign keys for a table.
pub fn list_foreign_keys(
    conn: &oracle::Connection,
    owner: &str,
    table: &str,
) -> Result<Vec<ForeignKeyInfo>, RowmanceError> {
    let owner_upper = owner.to_uppercase();
    let table_upper = table.to_uppercase();

    let sql = "
        SELECT
            ac.constraint_name,
            acc.column_name,
            rcc.table_name AS ref_table,
            rcc.column_name AS ref_col,
            acc.position,
            ac.delete_rule
        FROM all_constraints ac
        JOIN all_cons_columns acc
            ON acc.owner = ac.owner AND acc.constraint_name = ac.constraint_name
        JOIN all_constraints rc
            ON rc.constraint_name = ac.r_constraint_name AND rc.owner = ac.r_owner
        JOIN all_cons_columns rcc
            ON rcc.constraint_name = rc.constraint_name AND rcc.owner = rc.owner
            AND rcc.position = acc.position
        WHERE ac.owner = :1 AND ac.table_name = :2 AND ac.constraint_type = 'R'
        ORDER BY ac.constraint_name, acc.position
    ";
    let mut stmt = conn
        .statement(sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper, &table_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut map: std::collections::BTreeMap<String, ForeignKeyInfo> =
        std::collections::BTreeMap::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let cname: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let ref_table: String = row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let ref_col: String = row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let delete_rule: String = row.get(5).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let entry = map.entry(cname.clone()).or_insert_with(|| ForeignKeyInfo {
            constraint_name: cname.clone(),
            columns: vec![],
            referenced_table: ref_table,
            referenced_columns: vec![],
            on_delete: delete_rule,
            on_update: "NO ACTION".to_string(),
        });
        entry.columns.push(col);
        entry.referenced_columns.push(ref_col);
    }
    Ok(map.into_values().collect())
}

/// List all views in the given schema (Oracle owner), including their text.
/// `TEXT` is a `LONG` column; the oracle crate fetches it as a `String` the
/// same way `get_ddl` fetches the CLOB result of `DBMS_METADATA.GET_DDL`.
pub fn list_views(conn: &oracle::Connection, owner: &str) -> Result<Vec<ViewInfo>, RowmanceError> {
    let owner_upper = owner.to_uppercase();
    let sql = "
        SELECT view_name, text
        FROM all_views
        WHERE owner = :1
        ORDER BY view_name
    ";
    let mut stmt = conn
        .statement(sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut views = Vec::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let definition: Option<String> =
            row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        views.push(ViewInfo {
            name,
            definition: definition.unwrap_or_default(),
        });
    }
    Ok(views)
}

/// List indexes for every table in the given schema (Oracle owner) in one query.
pub fn list_all_indexes(
    conn: &oracle::Connection,
    owner: &str,
) -> Result<Vec<(String, IndexInfo)>, RowmanceError> {
    let owner_upper = owner.to_uppercase();

    let sql = "
        SELECT
            i.table_name,
            i.index_name,
            ic.column_name,
            CASE i.uniqueness WHEN 'UNIQUE' THEN 1 ELSE 0 END AS is_unique,
            i.index_type
        FROM all_indexes i
        JOIN all_ind_columns ic
            ON ic.index_owner = i.owner AND ic.index_name = i.index_name
        WHERE i.owner = :1
        ORDER BY i.table_name, i.index_name, ic.column_position
    ";
    let mut stmt = conn
        .statement(sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut map: std::collections::BTreeMap<(String, String), IndexInfo> =
        std::collections::BTreeMap::new();
    let mut order: Vec<(String, String)> = Vec::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let table_name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let name: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let unique_i: i64 = row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let itype: String = row.get(4).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let key = (table_name, name.clone());
        if !map.contains_key(&key) {
            order.push(key.clone());
        }
        let entry = map.entry(key).or_insert_with(|| IndexInfo {
            name: name.clone(),
            columns: vec![],
            unique: unique_i == 1,
            index_type: itype,
        });
        entry.columns.push(col);
    }
    Ok(order
        .into_iter()
        .filter_map(|key| map.remove(&key).map(|idx| (key.0, idx)))
        .collect())
}

/// List foreign keys for every table in the given schema (Oracle owner) in one query.
pub fn list_all_foreign_keys(
    conn: &oracle::Connection,
    owner: &str,
) -> Result<Vec<(String, ForeignKeyInfo)>, RowmanceError> {
    let owner_upper = owner.to_uppercase();

    let sql = "
        SELECT
            ac.table_name,
            ac.constraint_name,
            acc.column_name,
            rcc.table_name AS ref_table,
            rcc.column_name AS ref_col,
            acc.position,
            ac.delete_rule
        FROM all_constraints ac
        JOIN all_cons_columns acc
            ON acc.owner = ac.owner AND acc.constraint_name = ac.constraint_name
        JOIN all_constraints rc
            ON rc.constraint_name = ac.r_constraint_name AND rc.owner = ac.r_owner
        JOIN all_cons_columns rcc
            ON rcc.constraint_name = rc.constraint_name AND rcc.owner = rc.owner
            AND rcc.position = acc.position
        WHERE ac.owner = :1 AND ac.constraint_type = 'R'
        ORDER BY ac.table_name, ac.constraint_name, acc.position
    ";
    let mut stmt = conn
        .statement(sql)
        .build()
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    let rows = stmt
        .query(&[&owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;

    let mut map: std::collections::BTreeMap<(String, String), ForeignKeyInfo> =
        std::collections::BTreeMap::new();
    let mut order: Vec<(String, String)> = Vec::new();
    for row_result in rows {
        let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let table_name: String = row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let cname: String = row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let col: String = row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let ref_table: String = row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let ref_col: String = row.get(4).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let delete_rule: String = row.get(6).map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let key = (table_name, cname.clone());
        if !map.contains_key(&key) {
            order.push(key.clone());
        }
        let entry = map.entry(key).or_insert_with(|| ForeignKeyInfo {
            constraint_name: cname.clone(),
            columns: vec![],
            referenced_table: ref_table,
            referenced_columns: vec![],
            on_delete: delete_rule,
            on_update: "NO ACTION".to_string(),
        });
        entry.columns.push(col);
        entry.referenced_columns.push(ref_col);
    }
    Ok(order
        .into_iter()
        .filter_map(|key| map.remove(&key).map(|fk| (key.0, fk)))
        .collect())
}

/// Count rows in a table.
pub fn count_table(
    conn: &oracle::Connection,
    owner: &str,
    table: &str,
) -> Result<i64, RowmanceError> {
    let owner_upper = owner.to_uppercase();
    let table_upper = table.to_uppercase();
    let sql = format!(
        r#"SELECT COUNT(*) FROM "{}"."{}" "#,
        owner_upper.replace('"', "\"\""),
        table_upper.replace('"', "\"\"")
    );
    let row = conn
        .query_row_as::<i64>(&sql, &[])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    Ok(row)
}

/// Oracle stores NOT NULL column constraints as `all_constraints` rows with
/// `constraint_type = 'C'` too (there's no separate NOT NULL constraint
/// type), using a system-generated name and a `search_condition` of the form
/// `"COL" IS NOT NULL`. These are not real check constraints and must be
/// filtered out here, otherwise every NOT NULL column in the schema would
/// show up as a spurious "check constraint" when diffing schemas.
fn is_synthetic_not_null_constraint(generated: &str, search_condition: &str) -> bool {
    if !generated.eq_ignore_ascii_case("GENERATED NAME") {
        return false;
    }
    // Collapse internal whitespace so formatting variations don't matter.
    let normalized = search_condition.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = normalized.to_uppercase();
    let Some(prefix) = upper.strip_suffix("IS NOT NULL") else {
        return false;
    };
    let prefix = prefix.trim();
    let col = prefix.trim_matches('"');
    !col.is_empty()
        && col
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '#')
}

/// List CHECK constraints. Pass `table: Some(name)` to scope to a single
/// table, or `None` to fetch every table's check constraints in the schema
/// (Oracle owner). Filters out the synthetic NOT NULL "check constraints"
/// Oracle generates for every NOT NULL column (see
/// `is_synthetic_not_null_constraint`).
pub fn list_check_constraints(
    conn: &oracle::Connection,
    owner: &str,
    table: Option<&str>,
) -> Result<Vec<CheckConstraintInfo>, RowmanceError> {
    let owner_upper = owner.to_uppercase();

    struct Raw {
        table_name: String,
        constraint_name: String,
        generated: String,
        search_condition: Option<String>,
    }

    let raws: Vec<Raw> = if let Some(table) = table {
        let table_upper = table.to_uppercase();
        let sql = "
            SELECT
                ac.table_name,
                ac.constraint_name,
                ac.generated,
                ac.search_condition
            FROM all_constraints ac
            WHERE ac.owner = :1 AND ac.table_name = :2 AND ac.constraint_type = 'C'
            ORDER BY ac.table_name, ac.constraint_name
        ";
        let mut stmt = conn
            .statement(sql)
            .build()
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let rows = stmt
            .query(&[&owner_upper, &table_upper])
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let mut out = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
            out.push(Raw {
                table_name: row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                constraint_name: row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                generated: row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                search_condition: row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?,
            });
        }
        out
    } else {
        let sql = "
            SELECT
                ac.table_name,
                ac.constraint_name,
                ac.generated,
                ac.search_condition
            FROM all_constraints ac
            WHERE ac.owner = :1 AND ac.constraint_type = 'C'
            ORDER BY ac.table_name, ac.constraint_name
        ";
        let mut stmt = conn
            .statement(sql)
            .build()
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let rows = stmt
            .query(&[&owner_upper])
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let mut out = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
            out.push(Raw {
                table_name: row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                constraint_name: row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                generated: row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                search_condition: row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?,
            });
        }
        out
    };

    Ok(raws
        .into_iter()
        .filter_map(|r| {
            let expression = r.search_condition.unwrap_or_default();
            if is_synthetic_not_null_constraint(&r.generated, &expression) {
                return None;
            }
            Some(CheckConstraintInfo {
                constraint_name: r.constraint_name,
                table_name: r.table_name,
                expression,
            })
        })
        .collect())
}

/// Extracts just the `BEFORE`/`AFTER`/`INSTEAD OF` timing prefix from
/// Oracle's `all_triggers.trigger_type` column, which combines timing with
/// firing granularity (e.g. `"BEFORE EACH ROW"`, `"AFTER STATEMENT"`,
/// `"INSTEAD OF"`). Compound triggers report `"COMPOUND"`, which has no
/// timing prefix to extract — such triggers fall back to the raw value.
fn extract_oracle_trigger_timing(trigger_type: &str) -> String {
    let upper = trigger_type.to_uppercase();
    if upper.starts_with("BEFORE") {
        "BEFORE".to_string()
    } else if upper.starts_with("AFTER") {
        "AFTER".to_string()
    } else if upper.starts_with("INSTEAD OF") {
        "INSTEAD OF".to_string()
    } else {
        trigger_type.to_string()
    }
}

/// List triggers. Pass `table: Some(name)` to scope to a single table, or
/// `None` to fetch every table's triggers in the schema (Oracle owner).
///
/// `all_triggers` gives one row per trigger with `trigger_type` (timing +
/// granularity, e.g. `"BEFORE EACH ROW"`) and `triggering_event` (event,
/// already a single text field even for multi-event triggers like
/// `"INSERT OR UPDATE"` — no concatenation needed here). Unlike SQLite's
/// `sqlite_master.sql` or SQL Server's `OBJECT_DEFINITION()`, Oracle's
/// catalog does not expose a ready-made full `CREATE TRIGGER` statement, so
/// `definition` is assembled here from `trigger_type` + `triggering_event` +
/// the table name + `trigger_body` (fetched as `Option<String>` the same way
/// `list_views` reads the LONG `all_views.text` column above).
pub fn list_triggers(
    conn: &oracle::Connection,
    owner: &str,
    table: Option<&str>,
) -> Result<Vec<TriggerInfo>, RowmanceError> {
    let owner_upper = owner.to_uppercase();

    struct Raw {
        table_name: String,
        trigger_name: String,
        trigger_type: String,
        triggering_event: String,
        trigger_body: Option<String>,
    }

    let raws: Vec<Raw> = if let Some(table) = table {
        let table_upper = table.to_uppercase();
        let sql = "
            SELECT
                t.table_name,
                t.trigger_name,
                t.trigger_type,
                t.triggering_event,
                t.trigger_body
            FROM all_triggers t
            WHERE t.owner = :1 AND t.table_name = :2
            ORDER BY t.table_name, t.trigger_name
        ";
        let mut stmt = conn
            .statement(sql)
            .build()
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let rows = stmt
            .query(&[&owner_upper, &table_upper])
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let mut out = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
            out.push(Raw {
                table_name: row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                trigger_name: row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                trigger_type: row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                triggering_event: row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                trigger_body: row.get(4).map_err(|e| RowmanceError::Pool(e.to_string()))?,
            });
        }
        out
    } else {
        let sql = "
            SELECT
                t.table_name,
                t.trigger_name,
                t.trigger_type,
                t.triggering_event,
                t.trigger_body
            FROM all_triggers t
            WHERE t.owner = :1
            ORDER BY t.table_name, t.trigger_name
        ";
        let mut stmt = conn
            .statement(sql)
            .build()
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let rows = stmt
            .query(&[&owner_upper])
            .map_err(|e| RowmanceError::Pool(e.to_string()))?;
        let mut out = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| RowmanceError::Pool(e.to_string()))?;
            out.push(Raw {
                table_name: row.get(0).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                trigger_name: row.get(1).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                trigger_type: row.get(2).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                triggering_event: row.get(3).map_err(|e| RowmanceError::Pool(e.to_string()))?,
                trigger_body: row.get(4).map_err(|e| RowmanceError::Pool(e.to_string()))?,
            });
        }
        out
    };

    Ok(raws
        .into_iter()
        .map(|r| {
            let timing = extract_oracle_trigger_timing(&r.trigger_type);
            let body = r.trigger_body.unwrap_or_default();
            let definition = format!(
                "CREATE OR REPLACE TRIGGER \"{}\".\"{}\"\n{} {}\nON \"{}\".\"{}\"\n{}",
                owner_upper,
                r.trigger_name,
                r.trigger_type,
                r.triggering_event,
                owner_upper,
                r.table_name,
                body
            );
            TriggerInfo {
                name: r.trigger_name,
                table_name: r.table_name,
                timing,
                event: r.triggering_event,
                definition,
            }
        })
        .collect())
}

/// Get DDL for a table using DBMS_METADATA.
pub fn get_ddl(
    conn: &oracle::Connection,
    owner: &str,
    table: &str,
) -> Result<String, RowmanceError> {
    let owner_upper = owner.to_uppercase();
    let table_upper = table.to_uppercase();
    let sql = "SELECT DBMS_METADATA.GET_DDL('TABLE', :1, :2) FROM DUAL";
    let row = conn
        .query_row_as::<String>(sql, &[&table_upper, &owner_upper])
        .map_err(|e| RowmanceError::Pool(e.to_string()))?;
    Ok(row)
}

/// Convert an oracle Row value at position `idx` to serde_json::Value.
pub fn oracle_value_to_json(row: &oracle::Row, idx: usize) -> serde_json::Value {
    // Try i64 first
    if let Ok(Some(v)) = row.get::<usize, Option<i64>>(idx) {
        return serde_json::Value::Number(v.into());
    }
    // Try f64
    if let Ok(Some(v)) = row.get::<usize, Option<f64>>(idx) {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return serde_json::Value::Number(n);
        }
    }
    // Try String
    if let Ok(Some(v)) = row.get::<usize, Option<String>>(idx) {
        return serde_json::Value::String(v);
    }
    // Try bytes -> hex string
    if let Ok(Some(v)) = row.get::<usize, Option<Vec<u8>>>(idx) {
        let hex: String = v.iter().map(|b| format!("{:02x}", b)).collect();
        return serde_json::Value::String(hex);
    }
    serde_json::Value::Null
}

/// Convert a serde_json::Value to a String for binding to Oracle parameters.
pub fn json_value_to_oracle_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(if *b { "1".to_string() } else { "0".to_string() }),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

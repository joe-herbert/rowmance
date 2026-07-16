/// Shared ERD helper types and functions used by all engine implementations.
use crate::connections::types::{ErdColumn, ErdRelation, ErdTable};

/// Flat FK row in normalised form, ready for edge grouping.
pub(crate) struct FkNorm {
    pub constraint_name: String,
    pub table_name: String,
    pub column_name: String,
    pub referenced_table_name: String,
    pub referenced_column_name: String,
}

/// Fold an ordered iterator of (table_name, column) pairs into ErdTable nodes,
/// preserving the original ordering from the database query.
pub(crate) fn group_into_tables(rows: impl Iterator<Item = (String, ErdColumn)>) -> Vec<ErdTable> {
    let mut nodes: Vec<ErdTable> = Vec::new();
    for (table_name, col) in rows {
        if let Some(last) = nodes.last_mut() {
            if last.name == table_name {
                last.columns.push(col);
                continue;
            }
        }
        nodes.push(ErdTable {
            name: table_name,
            columns: vec![col],
        });
    }
    nodes
}

/// Accumulate FK rows (already sorted by constraint_name, ordinal) into ErdRelation edges.
pub(crate) fn build_edges_from_fk_rows(rows: impl Iterator<Item = FkNorm>) -> Vec<ErdRelation> {
    let mut edges: Vec<ErdRelation> = Vec::new();
    for row in rows {
        if let Some(last) = edges.last_mut() {
            if last.constraint_name == row.constraint_name {
                last.from_columns.push(row.column_name);
                last.to_columns.push(row.referenced_column_name);
                continue;
            }
        }
        edges.push(ErdRelation {
            from_table: row.table_name,
            from_columns: vec![row.column_name],
            to_table: row.referenced_table_name,
            to_columns: vec![row.referenced_column_name],
            constraint_name: row.constraint_name,
        });
    }
    edges
}

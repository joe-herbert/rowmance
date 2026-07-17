/// Tauri commands for Entity-Relationship Diagram data.
use std::sync::Arc;
use tauri::State;

use crate::connections::pool_manager::ConnectionManager;
use crate::connections::types::ErdGraph;
use crate::error::AppError;

#[tauri::command]
pub async fn erd_get_graph(
    connections: State<'_, Arc<ConnectionManager>>,
    connection_id: String,
    database: String,
    instance_db: Option<String>,
) -> Result<ErdGraph, AppError> {
    let engine = connections.get_engine(&connection_id).map_err(AppError::from)?;
    engine.get_erd_graph(&database, instance_db.as_deref()).await.map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use crate::connections::erd::{build_edges_from_fk_rows, group_into_tables, FkNorm};
    use crate::connections::types::ErdColumn;

    fn col(name: &str, pk: bool) -> ErdColumn {
        ErdColumn {
            name: name.to_owned(),
            data_type: "int".to_owned(),
            is_primary_key: pk,
        }
    }

    fn fk(
        constraint: &str,
        from_table: &str,
        from_col: &str,
        to_table: &str,
        to_col: &str,
    ) -> FkNorm {
        FkNorm {
            constraint_name: constraint.to_owned(),
            table_name: from_table.to_owned(),
            column_name: from_col.to_owned(),
            referenced_table_name: to_table.to_owned(),
            referenced_column_name: to_col.to_owned(),
        }
    }

    #[test]
    fn group_into_tables_empty() {
        let nodes = group_into_tables(std::iter::empty());
        assert!(nodes.is_empty());
    }

    #[test]
    fn group_into_tables_single_table_multiple_columns() {
        let rows = vec![
            ("users".to_owned(), col("id", true)),
            ("users".to_owned(), col("name", false)),
        ];
        let nodes = group_into_tables(rows.into_iter());
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "users");
        assert_eq!(nodes[0].columns.len(), 2);
        assert!(nodes[0].columns[0].is_primary_key);
    }

    #[test]
    fn group_into_tables_multiple_tables() {
        let rows = vec![
            ("orders".to_owned(), col("id", true)),
            ("users".to_owned(), col("id", true)),
            ("users".to_owned(), col("email", false)),
        ];
        let nodes = group_into_tables(rows.into_iter());
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "orders");
        assert_eq!(nodes[1].name, "users");
        assert_eq!(nodes[1].columns.len(), 2);
    }

    #[test]
    fn build_edges_empty() {
        let edges = build_edges_from_fk_rows(std::iter::empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn build_edges_single_column_fk() {
        let rows = vec![fk("fk_a", "orders", "user_id", "users", "id")];
        let edges = build_edges_from_fk_rows(rows.into_iter());
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].constraint_name, "fk_a");
        assert_eq!(edges[0].from_table, "orders");
        assert_eq!(edges[0].from_columns, vec!["user_id"]);
        assert_eq!(edges[0].to_table, "users");
        assert_eq!(edges[0].to_columns, vec!["id"]);
    }

    #[test]
    fn build_edges_composite_fk_merged() {
        let rows = vec![
            fk("fk_comp", "items", "order_id", "orders", "id"),
            fk("fk_comp", "items", "product_id", "orders", "product_id"),
        ];
        let edges = build_edges_from_fk_rows(rows.into_iter());
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_columns, vec!["order_id", "product_id"]);
        assert_eq!(edges[0].to_columns, vec!["id", "product_id"]);
    }

    #[test]
    fn build_edges_distinct_constraints_not_merged() {
        let rows = vec![
            fk("fk_a", "child", "a_id", "parent_a", "id"),
            fk("fk_b", "child", "b_id", "parent_b", "id"),
        ];
        let edges = build_edges_from_fk_rows(rows.into_iter());
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].constraint_name, "fk_a");
        assert_eq!(edges[1].constraint_name, "fk_b");
    }
}

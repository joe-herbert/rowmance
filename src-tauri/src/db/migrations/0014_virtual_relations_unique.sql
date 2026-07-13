CREATE UNIQUE INDEX IF NOT EXISTS uq_virtual_relations_from_to
ON virtual_relations (
    from_connection_id, from_database, from_table, from_column,
    to_connection_id, to_database, to_table, to_column
);

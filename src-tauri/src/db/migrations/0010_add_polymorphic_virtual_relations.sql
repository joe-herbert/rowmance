CREATE TABLE IF NOT EXISTS polymorphic_virtual_relations (
    id TEXT PRIMARY KEY,
    label TEXT,
    connection_id TEXT NOT NULL,
    database TEXT NOT NULL,
    table_name TEXT NOT NULL,
    type_column TEXT NOT NULL,
    value_column TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS polymorphic_vr_mappings (
    id TEXT PRIMARY KEY,
    relation_id TEXT NOT NULL REFERENCES polymorphic_virtual_relations(id) ON DELETE CASCADE,
    type_value TEXT NOT NULL,
    to_connection_id TEXT NOT NULL,
    to_database TEXT NOT NULL,
    to_table TEXT NOT NULL,
    to_column TEXT NOT NULL
);

PRAGMA foreign_keys = ON;

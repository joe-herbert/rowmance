CREATE TABLE IF NOT EXISTS virtual_relations (
    id TEXT PRIMARY KEY,
    label TEXT,
    from_connection_id TEXT NOT NULL,
    from_database TEXT NOT NULL,
    from_table TEXT NOT NULL,
    from_column TEXT NOT NULL,
    to_connection_id TEXT NOT NULL,
    to_database TEXT NOT NULL,
    to_table TEXT NOT NULL,
    to_column TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

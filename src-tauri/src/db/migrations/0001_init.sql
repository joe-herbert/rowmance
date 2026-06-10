-- Initial schema for the Rowmance local config database.

CREATE TABLE connection_groups (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_id   TEXT REFERENCES connection_groups(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE connection_profiles (
    id              TEXT PRIMARY KEY,
    group_id        TEXT REFERENCES connection_groups(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    db_type         TEXT NOT NULL CHECK (db_type IN ('mysql', 'mariadb', 'postgres')),
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL,
    database        TEXT NOT NULL,
    username        TEXT NOT NULL,
    color           TEXT,
    read_only       INTEGER NOT NULL DEFAULT 0,
    -- SSH tunnel
    ssh_enabled     INTEGER NOT NULL DEFAULT 0,
    ssh_host        TEXT,
    ssh_port        INTEGER DEFAULT 22,
    ssh_user        TEXT,
    ssh_auth_type   TEXT CHECK (ssh_auth_type IN ('password', 'key', NULL)),
    ssh_key_path    TEXT,
    -- SSL/TLS
    ssl_enabled     INTEGER NOT NULL DEFAULT 0,
    ssl_ca_path     TEXT,
    ssl_cert_path   TEXT,
    ssl_key_path    TEXT,
    -- Connection pool
    pool_min        INTEGER NOT NULL DEFAULT 1,
    pool_max        INTEGER NOT NULL DEFAULT 5,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE query_history (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT NOT NULL REFERENCES connection_profiles(id) ON DELETE CASCADE,
    sql             TEXT NOT NULL,
    executed_at     TEXT NOT NULL DEFAULT (datetime('now')),
    duration_ms     INTEGER,
    row_count       INTEGER,
    error           TEXT,
    status          TEXT NOT NULL CHECK (status IN ('success', 'error', 'cancelled'))
);
CREATE INDEX idx_query_history_connection ON query_history(connection_id, executed_at DESC);

CREATE TABLE saved_query_folders (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_id   TEXT REFERENCES saved_query_folders(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE saved_queries (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT REFERENCES connection_profiles(id) ON DELETE SET NULL,
    folder_id       TEXT REFERENCES saved_query_folders(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    sql             TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE settings (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

CREATE TABLE keyboard_shortcut_overrides (
    action      TEXT PRIMARY KEY,
    shortcut    TEXT NOT NULL,
    preset      TEXT NOT NULL DEFAULT 'vscode'
);

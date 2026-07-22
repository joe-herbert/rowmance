-- Extend the allowed db_type values to include 'oracle'.
-- SQLite does not support modifying CHECK constraints via ALTER TABLE,
-- so we recreate the table with the updated constraint.
PRAGMA foreign_keys = OFF;

CREATE TABLE connection_profiles_new (
    id              TEXT PRIMARY KEY,
    group_id        TEXT REFERENCES connection_groups(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    db_type         TEXT NOT NULL CHECK (db_type IN ('mysql', 'mariadb', 'postgres', 'sqlite', 'sqlserver', 'oracle')),
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL,
    database        TEXT NOT NULL,
    username        TEXT NOT NULL,
    color           TEXT,
    read_only       INTEGER NOT NULL DEFAULT 0,
    ssh_enabled     INTEGER NOT NULL DEFAULT 0,
    ssh_host        TEXT,
    ssh_port        INTEGER DEFAULT 22,
    ssh_user        TEXT,
    ssh_auth_type   TEXT CHECK (ssh_auth_type IN ('password', 'key', NULL)),
    ssh_key_path    TEXT,
    ssl_enabled     INTEGER NOT NULL DEFAULT 0,
    ssl_ca_path     TEXT,
    ssl_cert_path   TEXT,
    ssl_key_path    TEXT,
    pool_max        INTEGER NOT NULL DEFAULT 5,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    password        TEXT,
    ping_interval   INTEGER
);

INSERT INTO connection_profiles_new SELECT * FROM connection_profiles;
DROP TABLE connection_profiles;
ALTER TABLE connection_profiles_new RENAME TO connection_profiles;

PRAGMA foreign_keys = ON;

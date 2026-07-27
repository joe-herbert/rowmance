-- Tags that can be attached to connections (many-to-many).

CREATE TABLE tags (
    id       TEXT PRIMARY KEY,
    name     TEXT NOT NULL UNIQUE,
    color    TEXT,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE connection_tags (
    connection_id TEXT NOT NULL REFERENCES connection_profiles(id) ON DELETE CASCADE,
    tag_id        TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (connection_id, tag_id)
);
CREATE INDEX idx_connection_tags_tag ON connection_tags(tag_id);

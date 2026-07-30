CREATE TABLE ai_conversations_new (
    id              TEXT PRIMARY KEY,
    mode            TEXT NOT NULL CHECK (mode IN ('generate','explain','describe','summarise','chat')),
    context_key     TEXT,
    title           TEXT NOT NULL,
    connection_id   TEXT REFERENCES connection_profiles(id) ON DELETE SET NULL,
    database        TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO ai_conversations_new SELECT * FROM ai_conversations;

DROP TABLE ai_conversations;

ALTER TABLE ai_conversations_new RENAME TO ai_conversations;

CREATE INDEX idx_ai_conversations_context ON ai_conversations(mode, context_key, connection_id, database);
CREATE INDEX idx_ai_conversations_updated ON ai_conversations(updated_at DESC);

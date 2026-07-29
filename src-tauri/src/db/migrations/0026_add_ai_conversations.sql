CREATE TABLE ai_conversations (
    id              TEXT PRIMARY KEY,
    mode            TEXT NOT NULL CHECK (mode IN ('generate','explain','describe','summarise')),
    context_key     TEXT,
    title           TEXT NOT NULL,
    connection_id   TEXT REFERENCES connection_profiles(id) ON DELETE SET NULL,
    database        TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_ai_conversations_context ON ai_conversations(mode, context_key, connection_id, database);
CREATE INDEX idx_ai_conversations_updated ON ai_conversations(updated_at DESC);

CREATE TABLE ai_messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('user','assistant')),
    content         TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_ai_messages_conversation ON ai_messages(conversation_id, created_at);

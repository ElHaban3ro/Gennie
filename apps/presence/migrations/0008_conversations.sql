CREATE TABLE presence_conversations (
    id INTEGER PRIMARY KEY,
    provider TEXT,
    provider_thread_id TEXT,
    title TEXT NOT NULL CHECK (length(title) > 0),
    automatic_title INTEGER NOT NULL DEFAULT 1 CHECK (automatic_title IN (0,1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK ((provider IS NULL AND provider_thread_id IS NULL) OR (provider IS NOT NULL AND provider_thread_id IS NOT NULL AND length(provider) > 0 AND length(provider_thread_id) > 0)),
    UNIQUE(provider, provider_thread_id)
);
INSERT INTO presence_conversations(id,title,automatic_title,created_at,updated_at)
SELECT 1,'Historial anterior',0,min(created_at),max(created_at) FROM presence_history HAVING count(*) > 0;
ALTER TABLE presence_history RENAME TO presence_history_previous;
CREATE TABLE presence_history (
    id INTEGER PRIMARY KEY,
    conversation_id INTEGER NOT NULL REFERENCES presence_conversations(id),
    speaker TEXT NOT NULL CHECK (speaker IN ('user','assistant','system')),
    text TEXT NOT NULL CHECK (length(text) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO presence_history(id,conversation_id,speaker,text,created_at)
SELECT id,1,speaker,text,created_at FROM presence_history_previous;
DROP TABLE presence_history_previous;
CREATE INDEX presence_history_conversation ON presence_history(conversation_id,id);
CREATE INDEX presence_conversations_activity ON presence_conversations(updated_at DESC,id DESC);
CREATE TABLE presence_current_conversation (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    conversation_id INTEGER NOT NULL REFERENCES presence_conversations(id)
);

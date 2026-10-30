-- IDs estables también después de eliminar el último mensaje del historial.
ALTER TABLE presence_history RENAME TO presence_history_before_replies;
CREATE TABLE presence_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES presence_conversations(id),
    speaker TEXT NOT NULL CHECK (speaker IN ('user','assistant','system')),
    text TEXT NOT NULL CHECK (length(text) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO presence_history(id,conversation_id,speaker,text,created_at)
SELECT id,conversation_id,speaker,text,created_at FROM presence_history_before_replies;
DROP TABLE presence_history_before_replies;
CREATE INDEX presence_history_conversation ON presence_history(conversation_id,id);
CREATE INDEX presence_history_timeline ON presence_history(created_at DESC,id DESC);
CREATE TABLE presence_message_replies (
    message_id INTEGER PRIMARY KEY REFERENCES presence_history(id) ON DELETE CASCADE,
    source_message_id INTEGER REFERENCES presence_history(id) ON DELETE SET NULL,
    speaker TEXT NOT NULL CHECK (speaker IN ('user','assistant','system')),
    text TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 4096),
    quoted_created_at TEXT NOT NULL,
    truncated INTEGER NOT NULL CHECK (truncated IN (0,1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX presence_reply_sources ON presence_message_replies(source_message_id);

CREATE TABLE presence_message_deliveries (
    event_key TEXT PRIMARY KEY,
    conversation_id INTEGER NOT NULL REFERENCES presence_conversations(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

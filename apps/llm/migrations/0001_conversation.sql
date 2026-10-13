CREATE TABLE IF NOT EXISTS codex_conversation (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    thread_id TEXT NOT NULL CHECK (length(thread_id) > 0),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

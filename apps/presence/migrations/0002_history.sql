CREATE TABLE presence_history (
    id INTEGER PRIMARY KEY,
    speaker TEXT NOT NULL CHECK (speaker IN ('user', 'assistant', 'system')),
    text TEXT NOT NULL CHECK (length(text) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

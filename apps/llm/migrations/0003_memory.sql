CREATE TABLE llm_memory (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL CHECK (length(trim(text)) BETWEEN 1 AND 1200),
    source TEXT NOT NULL CHECK (length(trim(source)) BETWEEN 1 AND 1200),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX llm_memory_updated ON llm_memory(updated_at DESC, id DESC);

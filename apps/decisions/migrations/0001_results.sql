CREATE TABLE decision_attempts (
    id INTEGER PRIMARY KEY,
    action TEXT NOT NULL,
    target TEXT,
    status TEXT NOT NULL CHECK(status IN ('pending','verified','failed','uncertain')),
    evidence TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

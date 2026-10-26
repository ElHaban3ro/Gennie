CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    request_key TEXT NOT NULL UNIQUE CHECK(length(request_key) BETWEEN 1 AND 120),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    document TEXT NOT NULL CHECK(json_valid(document)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

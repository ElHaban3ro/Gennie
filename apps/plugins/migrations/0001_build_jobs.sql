CREATE TABLE plugin_build_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_key TEXT NOT NULL UNIQUE,
    plugin_id TEXT NOT NULL,
    origin TEXT NOT NULL,
    objective TEXT NOT NULL,
    trigger_conversation_id INTEGER NOT NULL,
    base_revision INTEGER NOT NULL CHECK(base_revision >= 0),
    state TEXT NOT NULL CHECK(state IN ('queued','running','waiting_user','completed','blocked','cancelled','interrupted')),
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL CHECK(max_attempts BETWEEN 1 AND 8),
    max_seconds INTEGER NOT NULL CHECK(max_seconds BETWEEN 30 AND 1800),
    spent_seconds INTEGER NOT NULL DEFAULT 0,
    thread_id TEXT,
    summary TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE UNIQUE INDEX plugin_build_one_live_target ON plugin_build_jobs(plugin_id) WHERE state IN ('queued','running','waiting_user','interrupted');
CREATE TABLE plugin_build_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL REFERENCES plugin_build_jobs(id),
    message TEXT NOT NULL,
    delivered INTEGER NOT NULL DEFAULT 0 CHECK(delivered IN (0,1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

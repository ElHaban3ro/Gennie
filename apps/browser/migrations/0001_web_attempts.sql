CREATE TABLE browser_web_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plugin_id TEXT NOT NULL CHECK(length(plugin_id) BETWEEN 1 AND 64),
    plugin_name TEXT NOT NULL CHECK(length(plugin_name) BETWEEN 1 AND 200),
    function_name TEXT NOT NULL CHECK(length(function_name) BETWEEN 1 AND 64),
    plugin_revision INTEGER NOT NULL CHECK(plugin_revision > 0),
    candidate_id TEXT,
    operation TEXT NOT NULL CHECK(operation IN ('run','verify')),
    state TEXT NOT NULL DEFAULT 'running' CHECK(state IN ('running','completed','uncertain','reviewed')),
    reason TEXT NOT NULL DEFAULT 'reserved' CHECK(reason IN ('reserved','reported_success','reported_failure','transport_error','interrupted')),
    resolution TEXT CHECK(resolution IN ('applied','not_applied')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    reviewed_at TEXT,
    CHECK((state = 'reviewed') = (resolution IS NOT NULL)),
    CHECK((resolution IS NULL) = (reviewed_at IS NULL))
);
CREATE UNIQUE INDEX browser_web_attempt_pending ON browser_web_attempts(plugin_id,function_name) WHERE state IN ('running','uncertain');
CREATE INDEX browser_web_attempt_history ON browser_web_attempts(plugin_id,id);

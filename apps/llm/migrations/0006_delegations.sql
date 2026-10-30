CREATE TABLE llm_delegations (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    bot_id TEXT NOT NULL REFERENCES llm_architects(id),
    source_id TEXT NOT NULL REFERENCES llm_architects(id),
    origin TEXT NOT NULL,
    job_id INTEGER NOT NULL CHECK (job_id > 0),
    plugin_id TEXT NOT NULL,
    objective TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE llm_delegation_updates (
    event_key TEXT PRIMARY KEY,
    delegation_id TEXT NOT NULL REFERENCES llm_delegations(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    document TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','rendering','ready','delivered','discarded','superseded')),
    rendered_text TEXT,
    failure_code TEXT CHECK (failure_code IN ('interrupted','render_failed')),
    bot_revision INTEGER,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(delegation_id, sequence)
);
CREATE INDEX llm_delegation_pending ON llm_delegation_updates(state, created_at, event_key);

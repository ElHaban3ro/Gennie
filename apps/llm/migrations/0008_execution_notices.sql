CREATE TABLE llm_execution_notices (
    event_key TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL REFERENCES llm_architects(id),
    document TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','rendering','ready','delivered','discarded')),
    rendered_text TEXT,
    failure_code TEXT CHECK (failure_code IN ('interrupted','render_failed')),
    bot_revision INTEGER,
    render_config TEXT,
    render_attempted_at TEXT,
    render_duration_ms INTEGER CHECK (render_duration_ms IS NULL OR render_duration_ms >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX llm_execution_notice_pending ON llm_execution_notices(state,created_at,event_key);

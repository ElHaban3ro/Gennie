CREATE TABLE assignment_execution_notices (
    run_id INTEGER PRIMARY KEY REFERENCES assignment_execution_runs(id),
    document TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','accepted')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    accepted_at TEXT
);
CREATE INDEX assignment_execution_notice_pending ON assignment_execution_notices(state,run_id);

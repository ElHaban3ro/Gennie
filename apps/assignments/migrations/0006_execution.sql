CREATE TABLE assignment_execution_policies (
    id TEXT PRIMARY KEY NOT NULL,
    assignment_id INTEGER NOT NULL,
    instance_id TEXT NOT NULL,
    trigger TEXT NOT NULL CHECK (trigger IN ('delivery','preparation')),
    revision INTEGER NOT NULL CHECK (revision > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0,1)),
    state TEXT NOT NULL CHECK (state IN ('active','paused','stale','review_required','completed')),
    next_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(instance_id,trigger)
);
CREATE TABLE assignment_execution_versions (
    policy_id TEXT NOT NULL REFERENCES assignment_execution_policies(id),
    revision INTEGER NOT NULL,
    document TEXT NOT NULL,
    PRIMARY KEY(policy_id,revision)
);
CREATE TABLE assignment_execution_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    policy_id TEXT NOT NULL,
    policy_revision INTEGER NOT NULL,
    scheduled_at TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('reserved','running','succeeded','rejected','uncertain','cancelled','skipped')),
    code TEXT,
    output TEXT,
    output_omitted INTEGER NOT NULL DEFAULT 0 CHECK (output_omitted IN (0,1)),
    started_at TEXT,
    completed_at TEXT,
    resolution TEXT CHECK (resolution IN ('applied','not_applied')),
    reviewed_at TEXT,
    FOREIGN KEY(policy_id,policy_revision) REFERENCES assignment_execution_versions(policy_id,revision),
    UNIQUE(policy_id,scheduled_at)
);
CREATE INDEX assignment_execution_due ON assignment_execution_policies(enabled,next_at);
CREATE INDEX assignment_execution_activity ON assignment_execution_runs(policy_id,id DESC);

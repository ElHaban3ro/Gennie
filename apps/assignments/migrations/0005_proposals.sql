CREATE TABLE assignment_reviews (
    id TEXT PRIMARY KEY NOT NULL,
    request_key TEXT NOT NULL UNIQUE,
    request TEXT NOT NULL,
    scope TEXT NOT NULL,
    submission TEXT,
    state TEXT NOT NULL CHECK (state IN ('waiting_bot','evaluating','ready','guide_failed','interrupted')),
    guide TEXT,
    duration_ms INTEGER CHECK (duration_ms >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE assignment_proposals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_id TEXT NOT NULL REFERENCES assignment_reviews(id),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    candidate TEXT NOT NULL,
    resolution TEXT NOT NULL DEFAULT 'pending' CHECK (resolution IN ('pending','applied','dismissed')),
    applied_revision INTEGER,
    resolved_at TEXT,
    UNIQUE(review_id,ordinal)
);
CREATE INDEX assignment_review_activity ON assignment_reviews(updated_at DESC,id);

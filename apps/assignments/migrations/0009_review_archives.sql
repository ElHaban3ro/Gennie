CREATE TABLE assignment_review_archives (
    root_review_id TEXT PRIMARY KEY REFERENCES assignment_reviews(id),
    archived_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX assignment_review_created ON assignment_reviews(created_at DESC,id DESC);

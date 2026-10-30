CREATE TABLE assignment_review_inputs (
    review_id TEXT PRIMARY KEY REFERENCES assignment_reviews(id),
    message_id INTEGER NOT NULL CHECK(message_id > 0),
    message_text TEXT NOT NULL CHECK(length(message_text) BETWEEN 1 AND 4000),
    truncated INTEGER NOT NULL CHECK(truncated IN (0,1)),
    message_created_at TEXT NOT NULL
);
CREATE TABLE assignment_review_refinements (
    review_id TEXT PRIMARY KEY REFERENCES assignment_reviews(id),
    parent_review_id TEXT NOT NULL UNIQUE REFERENCES assignment_reviews(id),
    root_review_id TEXT NOT NULL REFERENCES assignment_reviews(id),
    round INTEGER NOT NULL CHECK(round > 1),
    message_id INTEGER NOT NULL CHECK(message_id > 0),
    message_text TEXT NOT NULL CHECK(length(message_text) BETWEEN 1 AND 4000),
    truncated INTEGER NOT NULL CHECK(truncated IN (0,1)),
    message_created_at TEXT NOT NULL,
    UNIQUE(root_review_id,message_id)
);
CREATE INDEX assignment_refinement_root ON assignment_review_refinements(root_review_id,round);

CREATE TABLE assignment_chat_review_requests (
    review_id TEXT PRIMARY KEY REFERENCES assignment_reviews(id),
    conversation_id INTEGER NOT NULL CHECK(conversation_id > 0),
    epoch INTEGER NOT NULL CHECK(epoch > 0),
    message_id INTEGER NOT NULL CHECK(message_id > 0),
    message_text TEXT NOT NULL CHECK(length(message_text) BETWEEN 1 AND 4000),
    truncated INTEGER NOT NULL CHECK(truncated IN (0,1)),
    message_created_at TEXT NOT NULL,
    UNIQUE(conversation_id,epoch,message_id)
);

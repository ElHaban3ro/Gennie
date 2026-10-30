CREATE TABLE presence_execution_messages (
    message_id INTEGER PRIMARY KEY REFERENCES presence_history(id) ON DELETE CASCADE,
    event_key TEXT NOT NULL UNIQUE,
    reference TEXT NOT NULL
);

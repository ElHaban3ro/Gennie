ALTER TABLE presence_conversations ADD COLUMN delivery_epoch INTEGER NOT NULL DEFAULT 1 CHECK (delivery_epoch > 0);
CREATE TABLE presence_delegation_messages (
    message_id INTEGER PRIMARY KEY REFERENCES presence_history(id) ON DELETE CASCADE,
    event_key TEXT NOT NULL UNIQUE,
    delegation_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    job_id INTEGER NOT NULL CHECK (job_id > 0),
    source_sequence INTEGER NOT NULL CHECK (source_sequence > 0),
    plugin_id TEXT NOT NULL
);

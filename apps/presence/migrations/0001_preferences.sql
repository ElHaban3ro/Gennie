CREATE TABLE presence_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    reduced_motion INTEGER NOT NULL DEFAULT 0 CHECK (reduced_motion IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO presence_preferences (id) VALUES (1);

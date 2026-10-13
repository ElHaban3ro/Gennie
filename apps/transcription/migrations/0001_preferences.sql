CREATE TABLE transcription_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    shortcut TEXT NOT NULL DEFAULT 'Ctrl+Option+Space',
    mode TEXT NOT NULL DEFAULT 'hold' CHECK (mode IN ('hold', 'toggle')),
    bubble_seconds INTEGER NOT NULL DEFAULT 5 CHECK (bubble_seconds BETWEEN 2 AND 30),
    unload_seconds INTEGER NOT NULL DEFAULT 30 CHECK (unload_seconds BETWEEN 0 AND 300),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO transcription_preferences(id) VALUES (1);

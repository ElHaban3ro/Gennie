CREATE TABLE transcription_preferences_next (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    shortcut TEXT NOT NULL DEFAULT 'Ctrl+Option+Space',
    mode TEXT NOT NULL DEFAULT 'hold' CHECK (mode IN ('hold', 'toggle')),
    bubble_seconds INTEGER NOT NULL DEFAULT 15 CHECK (bubble_seconds BETWEEN 2 AND 30),
    unload_seconds INTEGER NOT NULL DEFAULT 600 CHECK (unload_seconds BETWEEN 0 AND 600),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    microphone_name TEXT
);
INSERT INTO transcription_preferences_next
SELECT id,enabled,shortcut,mode,
    CASE WHEN bubble_seconds=5 THEN 15 ELSE bubble_seconds END,
    unload_seconds,
    CASE WHEN bubble_seconds=5 THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE updated_at END,
    microphone_name
FROM transcription_preferences;
DROP TABLE transcription_preferences;
ALTER TABLE transcription_preferences_next RENAME TO transcription_preferences;

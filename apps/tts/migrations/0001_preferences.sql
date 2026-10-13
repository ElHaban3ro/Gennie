CREATE TABLE tts_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    voice TEXT NOT NULL DEFAULT 'ef_dora' CHECK (voice IN ('ef_dora', 'em_alex', 'em_santa')),
    speed_percent INTEGER NOT NULL DEFAULT 100 CHECK (speed_percent BETWEEN 70 AND 130),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO tts_preferences (id) VALUES (1);

CREATE TABLE tts_preferences_new (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    voice TEXT NOT NULL DEFAULT 'ef_dora' CHECK (voice IN ('ef_dora', 'em_alex', 'em_santa')),
    speed_percent INTEGER NOT NULL DEFAULT 100 CHECK (speed_percent BETWEEN 50 AND 150),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    unload_seconds INTEGER DEFAULT 600 CHECK (unload_seconds IS NULL OR (unload_seconds BETWEEN 60 AND 7200 AND unload_seconds % 60 = 0)),
    provider TEXT NOT NULL DEFAULT 'kokoro' CHECK (provider IN ('kokoro', 'inworld')),
    inworld_voice TEXT NOT NULL DEFAULT 'Ashley',
    inworld_model TEXT NOT NULL DEFAULT 'inworld-tts-2-flash',
    inworld_language TEXT NOT NULL DEFAULT 'es-CO',
    inworld_delivery TEXT NOT NULL DEFAULT 'BALANCED',
    inworld_normalization TEXT NOT NULL DEFAULT 'AUTO'
);
INSERT INTO tts_preferences_new SELECT id, enabled, voice, speed_percent, updated_at, unload_seconds, provider, inworld_voice, inworld_model, inworld_language, inworld_delivery, inworld_normalization FROM tts_preferences;
DROP TABLE tts_preferences;
ALTER TABLE tts_preferences_new RENAME TO tts_preferences;

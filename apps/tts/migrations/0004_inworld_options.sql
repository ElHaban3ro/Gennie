ALTER TABLE tts_preferences ADD COLUMN inworld_model TEXT NOT NULL DEFAULT 'inworld-tts-2-flash';
ALTER TABLE tts_preferences ADD COLUMN inworld_language TEXT NOT NULL DEFAULT 'es-CO';
ALTER TABLE tts_preferences ADD COLUMN inworld_delivery TEXT NOT NULL DEFAULT 'BALANCED';
ALTER TABLE tts_preferences ADD COLUMN inworld_normalization TEXT NOT NULL DEFAULT 'AUTO';

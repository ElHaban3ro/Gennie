ALTER TABLE tts_preferences
ADD COLUMN unload_seconds INTEGER DEFAULT 600
CHECK (unload_seconds IS NULL OR (unload_seconds BETWEEN 60 AND 7200 AND unload_seconds % 60 = 0));

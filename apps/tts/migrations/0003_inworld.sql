ALTER TABLE tts_preferences ADD COLUMN provider TEXT NOT NULL DEFAULT 'kokoro' CHECK (provider IN ('kokoro', 'inworld'));
ALTER TABLE tts_preferences ADD COLUMN inworld_voice TEXT NOT NULL DEFAULT 'Ashley';

ALTER TABLE presence_conversations ADD COLUMN title_generation_attempted INTEGER NOT NULL DEFAULT 0 CHECK (title_generation_attempted IN (0,1));

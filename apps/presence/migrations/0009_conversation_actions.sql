ALTER TABLE presence_conversations ADD COLUMN provider_title TEXT CHECK (provider_title IS NULL OR length(trim(provider_title)) > 0);
ALTER TABLE presence_conversations ADD COLUMN local_title TEXT CHECK (local_title IS NULL OR length(trim(local_title)) BETWEEN 1 AND 200);
ALTER TABLE presence_conversations ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1));

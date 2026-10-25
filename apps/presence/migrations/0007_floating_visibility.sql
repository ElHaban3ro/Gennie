ALTER TABLE presence_preferences ADD COLUMN floating_visible INTEGER NOT NULL DEFAULT 0 CHECK (floating_visible IN (0, 1));

-- Solo las observaciones del DOM controladas por el host pueden recuperarse sin revisión de efectos.
ALTER TABLE browser_web_attempts ADD COLUMN observation_only INTEGER NOT NULL DEFAULT 0 CHECK(observation_only IN (0,1));

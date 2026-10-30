ALTER TABLE llm_delegation_updates ADD COLUMN render_config TEXT;
ALTER TABLE llm_delegation_updates ADD COLUMN render_attempted_at TEXT;
ALTER TABLE llm_delegation_updates ADD COLUMN render_duration_ms INTEGER CHECK (render_duration_ms IS NULL OR render_duration_ms >= 0);

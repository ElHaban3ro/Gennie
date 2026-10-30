ALTER TABLE llm_execution_notices ADD COLUMN guide_state TEXT NOT NULL DEFAULT 'not_requested' CHECK (guide_state IN ('not_requested','reserved','ready','failed','interrupted','unavailable','bypassed'));
ALTER TABLE llm_execution_notices ADD COLUMN guide_config TEXT;
ALTER TABLE llm_execution_notices ADD COLUMN guide_advice TEXT;
ALTER TABLE llm_execution_notices ADD COLUMN guide_attempted_at TEXT;
ALTER TABLE llm_execution_notices ADD COLUMN guide_duration_ms INTEGER CHECK (guide_duration_ms >= 0);
ALTER TABLE llm_execution_notices ADD COLUMN notice_action TEXT CHECK (notice_action IN ('record','notify','ask','offer_help'));
ALTER TABLE llm_execution_notices ADD COLUMN notice_reason TEXT;
CREATE INDEX llm_execution_notice_context ON llm_execution_notices(json_extract(document,'$.reference.policy_id'),json_extract(document,'$.reference.policy_revision'),json_extract(document,'$.origin.conversation_id'),json_extract(document,'$.origin.epoch'),json_extract(document,'$.reference.run_id'));

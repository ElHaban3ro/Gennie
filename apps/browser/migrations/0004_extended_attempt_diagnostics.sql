-- Códigos y fases adicionales; no se modifican diagnósticos históricos.
ALTER TABLE browser_web_attempts ADD COLUMN failure_code_extended TEXT CHECK(failure_code_extended IS NULL OR failure_code_extended IN ('result_type_mismatch'));
ALTER TABLE browser_web_attempts ADD COLUMN failure_phase_extended TEXT CHECK(failure_phase_extended IS NULL OR failure_phase_extended IN ('execute'));

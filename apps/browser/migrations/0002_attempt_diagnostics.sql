-- Diagnóstico técnico cerrado; nunca contiene argumentos ni datos del sitio.
ALTER TABLE browser_web_attempts ADD COLUMN failure_code TEXT CHECK(failure_code IS NULL OR failure_code IN ('timeout','connection_failed','type_error','reference_error','program_error','postcondition_failed'));
ALTER TABLE browser_web_attempts ADD COLUMN failure_phase TEXT CHECK(failure_phase IS NULL OR failure_phase IN ('prepare','read','action','verify'));

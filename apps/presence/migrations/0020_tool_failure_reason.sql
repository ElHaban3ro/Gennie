ALTER TABLE presence_tool_uses ADD COLUMN failure_reason TEXT CHECK (failure_reason IS NULL OR length(failure_reason) BETWEEN 7 AND 506);

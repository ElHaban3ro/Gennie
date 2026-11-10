-- Evidencia mínima de la conversación y cuenta, capturada antes del archivo.
CREATE TABLE browser_gmail_archive_evidence (
    attempt_id INTEGER PRIMARY KEY REFERENCES browser_web_attempts(id),
    account_hash TEXT NOT NULL CHECK(length(account_hash) = 64),
    thread_id TEXT NOT NULL CHECK(length(thread_id) BETWEEN 1 AND 16),
    unread_before INTEGER NOT NULL CHECK(unread_before IN (0, 1)),
    auto_resolution TEXT CHECK(auto_resolution IS NULL OR auto_resolution IN ('applied', 'not_applied')),
    observed_at TEXT
);

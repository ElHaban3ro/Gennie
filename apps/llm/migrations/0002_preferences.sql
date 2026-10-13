CREATE TABLE llm_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    read_files INTEGER NOT NULL DEFAULT 1 CHECK (read_files IN (0, 1)),
    write_files INTEGER NOT NULL DEFAULT 1 CHECK (write_files IN (0, 1)),
    run_commands INTEGER NOT NULL DEFAULT 1 CHECK (run_commands IN (0, 1)),
    local_actions INTEGER NOT NULL DEFAULT 1 CHECK (local_actions IN (0, 1)),
    model TEXT NOT NULL DEFAULT 'gpt-6-luna' CHECK (length(model) BETWEEN 1 AND 100),
    effort TEXT NOT NULL DEFAULT 'low' CHECK (effort IN ('low', 'medium', 'high', 'xhigh', 'max', 'ultra')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO llm_preferences (id) VALUES (1);

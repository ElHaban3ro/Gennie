CREATE TABLE llm_architects (
    id TEXT PRIMARY KEY,
    role TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    model TEXT NOT NULL CHECK (length(model) BETWEEN 1 AND 100),
    effort TEXT,
    personality TEXT NOT NULL DEFAULT '' CHECK (length(personality) <= 1000),
    instructions TEXT NOT NULL DEFAULT '' CHECK (length(instructions) <= 6000),
    read_files INTEGER NOT NULL CHECK (read_files IN (0, 1)),
    write_files INTEGER NOT NULL CHECK (write_files IN (0, 1) AND write_files <= read_files),
    run_commands INTEGER NOT NULL CHECK (run_commands IN (0, 1)),
    local_actions INTEGER NOT NULL CHECK (local_actions IN (0, 1) AND local_actions <= run_commands),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK (
        (id = 'gennie' AND role = 'bot' AND provider = 'codex_chatgpt' AND enabled = 1) OR
        (id = 'plugin_builder' AND role = 'helper' AND provider = 'codex_chatgpt') OR
        (id = 'proactivity_guide' AND role = 'guide' AND provider = 'openrouter_decisions')
    ),
    CHECK (
        (role IN ('bot', 'helper') AND effort IN ('low','medium','high','xhigh','max','ultra') AND effort IS NOT NULL) OR
        (role = 'guide' AND effort IS NULL AND model IN ('typesafe/jev-1.14','openai/gpt-6-luna-decisions')
            AND read_files = 0 AND write_files = 0 AND run_commands = 0 AND local_actions = 0)
    )
);
INSERT INTO llm_architects (id,role,provider,enabled,model,effort,read_files,write_files,run_commands,local_actions,updated_at)
SELECT 'gennie','bot','codex_chatgpt',1,model,effort,read_files,write_files,run_commands,local_actions,updated_at FROM llm_preferences WHERE id=1;
INSERT INTO llm_architects (id,role,provider,enabled,model,effort,read_files,write_files,run_commands,local_actions,updated_at)
SELECT 'plugin_builder','helper','codex_chatgpt',1,model,effort,read_files,write_files,run_commands,local_actions,updated_at FROM llm_preferences WHERE id=1;
INSERT INTO llm_architects (id,role,provider,enabled,model,effort,read_files,write_files,run_commands,local_actions)
VALUES ('proactivity_guide','guide','openrouter_decisions',0,'typesafe/jev-1.14',NULL,0,0,0,0);

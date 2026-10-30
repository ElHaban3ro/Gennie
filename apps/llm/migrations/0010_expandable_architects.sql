ALTER TABLE llm_architects ADD COLUMN name TEXT NOT NULL DEFAULT '' CHECK(length(name) <= 200);
UPDATE llm_architects SET name = CASE id
    WHEN 'gennie' THEN 'Gennie'
    WHEN 'plugin_builder' THEN 'Constructor de plugins'
    WHEN 'proactivity_guide' THEN 'Guía de proactividad'
END;

-- Conserva las identidades base y las FK de delegaciones/avisos existentes.
CREATE TABLE llm_architect_instances (
    id TEXT PRIMARY KEY CHECK(id NOT IN ('gennie','plugin_builder','proactivity_guide')),
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 200),
    role TEXT NOT NULL CHECK(length(role) BETWEEN 1 AND 80),
    provider TEXT NOT NULL CHECK(provider IN ('codex_chatgpt','openrouter_decisions')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    model TEXT NOT NULL CHECK(length(model) BETWEEN 1 AND 100),
    effort TEXT,
    personality TEXT NOT NULL DEFAULT '' CHECK(length(personality) <= 1000),
    instructions TEXT NOT NULL DEFAULT '' CHECK(length(instructions) <= 6000),
    read_files INTEGER NOT NULL CHECK(read_files IN (0,1)),
    write_files INTEGER NOT NULL CHECK(write_files IN (0,1) AND write_files <= read_files),
    run_commands INTEGER NOT NULL CHECK(run_commands IN (0,1)),
    local_actions INTEGER NOT NULL CHECK(local_actions IN (0,1) AND local_actions <= run_commands),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK (
        (provider = 'codex_chatgpt' AND effort IS NOT NULL AND effort IN ('low','medium','high','xhigh','max','ultra')) OR
        (provider = 'openrouter_decisions' AND effort IS NULL
            AND model IN ('typesafe/jev-1.14','openai/gpt-6-luna-decisions')
            AND read_files = 0 AND write_files = 0 AND run_commands = 0 AND local_actions = 0)
    )
);

CREATE VIEW llm_architect_profiles AS
SELECT id,name,role,provider,revision,enabled,model,effort,personality,instructions,read_files,write_files,run_commands,local_actions,updated_at FROM llm_architects
UNION ALL
SELECT id,name,role,provider,revision,enabled,model,effort,personality,instructions,read_files,write_files,run_commands,local_actions,updated_at FROM llm_architect_instances;

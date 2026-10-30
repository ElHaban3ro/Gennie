CREATE TABLE presence_assistants (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 100),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO presence_assistants(id,name) VALUES('gennie','Gennie');

CREATE TABLE presence_assistant_conversations (
    conversation_id INTEGER PRIMARY KEY REFERENCES presence_conversations(id),
    assistant_id TEXT NOT NULL REFERENCES presence_assistants(id)
);
INSERT INTO presence_assistant_conversations(conversation_id,assistant_id)
SELECT id,'gennie' FROM presence_conversations;
CREATE INDEX presence_assistant_sessions ON presence_assistant_conversations(assistant_id,conversation_id);
CREATE INDEX presence_history_timeline ON presence_history(created_at DESC,id DESC);

-- Mientras solo existe el bot inicial, toda sesión nueva pertenece a Gennie.
-- La asignación explícita de otros bots se incorporará al habilitar su creación.
CREATE TRIGGER presence_default_assistant AFTER INSERT ON presence_conversations
BEGIN
    INSERT INTO presence_assistant_conversations(conversation_id,assistant_id) VALUES(NEW.id,'gennie');
END;

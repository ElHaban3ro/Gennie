CREATE TABLE presence_provider_inputs (
    user_message_id INTEGER PRIMARY KEY REFERENCES presence_history(id) ON DELETE CASCADE,
    conversation_id INTEGER NOT NULL REFERENCES presence_conversations(id) ON DELETE CASCADE,
    delivery_epoch INTEGER NOT NULL,
    provider TEXT NOT NULL,
    provider_thread TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'reserved' CHECK (state IN ('reserved','accepted','uncertain')),
    older_omitted INTEGER NOT NULL DEFAULT 0 CHECK (older_omitted >= 0),
    unconfirmed_omitted INTEGER NOT NULL DEFAULT 0 CHECK (unconfirmed_omitted >= 0),
    warning_before_id INTEGER,
    warned_by_user_id INTEGER REFERENCES presence_history(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    accepted_at TEXT
);
CREATE TABLE presence_provider_input_messages (
    message_id INTEGER PRIMARY KEY REFERENCES presence_history(id) ON DELETE CASCADE,
    conversation_id INTEGER NOT NULL REFERENCES presence_conversations(id) ON DELETE CASCADE,
    user_message_id INTEGER REFERENCES presence_provider_inputs(user_message_id) ON DELETE CASCADE,
    included INTEGER,
    CHECK ((user_message_id IS NULL AND included IS NULL) OR (user_message_id IS NOT NULL AND included IS NOT NULL AND included IN (0,1)))
);
CREATE INDEX presence_provider_context_scope ON presence_provider_inputs(conversation_id,delivery_epoch,state,warned_by_user_id);
CREATE INDEX presence_provider_context_messages ON presence_provider_input_messages(user_message_id,included);

INSERT INTO presence_provider_input_messages(message_id,conversation_id)
SELECT h.id,h.conversation_id FROM presence_history h JOIN (
    SELECT message_id FROM presence_delegation_messages UNION SELECT message_id FROM presence_execution_messages
) m ON m.message_id=h.id;
CREATE INDEX presence_provider_context_pending ON presence_provider_input_messages(conversation_id,user_message_id,message_id);

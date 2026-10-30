-- Correlación del protocolo, independiente del contexto que conserva el proveedor.
ALTER TABLE presence_history ADD COLUMN provider_turn_id TEXT;
ALTER TABLE presence_history ADD COLUMN provider_item_id TEXT;
CREATE UNIQUE INDEX presence_message_provider_items
    ON presence_history(conversation_id,provider_turn_id,provider_item_id)
    WHERE provider_item_id IS NOT NULL;
CREATE INDEX presence_message_turns ON presence_history(conversation_id,provider_turn_id,speaker);
CREATE UNIQUE INDEX presence_user_provider_turns ON presence_history(conversation_id,provider_turn_id)
    WHERE speaker='user' AND provider_turn_id IS NOT NULL;
CREATE TABLE presence_tool_uses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_message_id INTEGER NOT NULL REFERENCES presence_history(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL REFERENCES presence_history(id) ON DELETE CASCADE,
    turn_id TEXT NOT NULL CHECK (length(turn_id) BETWEEN 1 AND 256),
    call_id TEXT NOT NULL CHECK (length(call_id) BETWEEN 1 AND 256),
    server TEXT NOT NULL CHECK (length(server) BETWEEN 1 AND 200),
    tool TEXT NOT NULL CHECK (length(tool) BETWEEN 1 AND 200),
    plugin_id TEXT CHECK (plugin_id IS NULL OR length(plugin_id) BETWEEN 1 AND 64),
    function_name TEXT CHECK (function_name IS NULL OR length(function_name) BETWEEN 1 AND 64),
    display_name TEXT CHECK (display_name IS NULL OR length(display_name) BETWEEN 1 AND 200),
    operation TEXT CHECK (operation IS NULL OR operation IN ('run','verify')),
    outcome TEXT NOT NULL CHECK (outcome IN ('running','completed','failed','unknown')),
    detail TEXT CHECK (detail IS NULL OR detail IN ('result_uncertain','timeout','browser_closed','response_too_large','authentication_required','access_denied','site_changed','redirect_blocked','redirect_limit','http_error','execution_failed','permission_required','revision_conflict','verification_required','plugin_not_found','function_not_found','unconfirmed','restart')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(user_message_id,turn_id,call_id)
);
CREATE INDEX presence_tools_by_message ON presence_tool_uses(message_id,id);

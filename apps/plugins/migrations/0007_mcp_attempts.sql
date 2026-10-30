-- Reserva previa al efecto y revisión humana; sin argumentos, resultados ni errores libres.
CREATE TABLE plugin_mcp_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id TEXT NOT NULL REFERENCES plugin_mcp_sources(id),
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    server_name TEXT NOT NULL CHECK (length(server_name) BETWEEN 1 AND 200),
    tool TEXT NOT NULL CHECK (length(tool) BETWEEN 1 AND 200),
    state TEXT NOT NULL DEFAULT 'running' CHECK (state IN ('running','completed','uncertain','reviewed')),
    reason TEXT NOT NULL DEFAULT 'awaiting_result' CHECK (reason IN ('awaiting_result','reported_success','transport_error','tool_error','interrupted')),
    resolution TEXT CHECK (resolution IN ('applied','not_applied')),
    reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK ((state = 'reviewed') = (resolution IS NOT NULL AND reviewed_at IS NOT NULL))
);
CREATE UNIQUE INDEX plugin_mcp_pending_tool ON plugin_mcp_attempts(source_id,tool)
    WHERE state IN ('running','uncertain');
CREATE INDEX plugin_mcp_source_attempt ON plugin_mcp_attempts(source_id,id DESC);

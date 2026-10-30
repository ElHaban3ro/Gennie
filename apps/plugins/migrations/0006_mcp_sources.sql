-- Configuración y permisos explícitos de servidores externos, sin resultados ni secretos.
CREATE TABLE plugin_mcp_sources (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 64),
    document TEXT NOT NULL CHECK (json_valid(document)),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)),
    deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    tools TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(tools) AND length(tools) <= 2097152),
    allowed_tools TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(allowed_tools)),
    checked_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

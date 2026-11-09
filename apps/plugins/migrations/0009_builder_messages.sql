CREATE TABLE plugin_builder_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL REFERENCES plugin_build_jobs(id),
    request_key TEXT NOT NULL UNIQUE,
    origin TEXT NOT NULL,
    message TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('pending','sending','delivered','uncertain')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX plugin_builder_messages_job_state ON plugin_builder_messages(job_id,state,id);

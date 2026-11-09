CREATE TABLE plugin_hidden_jobs (
    job_id INTEGER PRIMARY KEY REFERENCES plugin_build_jobs(id)
);

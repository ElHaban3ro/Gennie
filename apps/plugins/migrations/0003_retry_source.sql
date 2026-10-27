ALTER TABLE plugin_build_jobs ADD COLUMN retry_of INTEGER REFERENCES plugin_build_jobs(id);

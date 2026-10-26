DROP INDEX plugin_build_one_live_target;
CREATE UNIQUE INDEX plugin_build_one_running_target ON plugin_build_jobs(plugin_id) WHERE state = 'running';
ALTER TABLE plugin_build_jobs ADD COLUMN refresh_base INTEGER NOT NULL DEFAULT 0 CHECK(refresh_base IN (0,1));

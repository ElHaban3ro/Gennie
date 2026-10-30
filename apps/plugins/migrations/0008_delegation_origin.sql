ALTER TABLE plugin_build_jobs ADD COLUMN delegation_origin TEXT;
ALTER TABLE plugin_build_jobs ADD COLUMN legacy_origin INTEGER NOT NULL DEFAULT 1 CHECK (legacy_origin IN (0,1));
ALTER TABLE plugin_build_events ADD COLUMN source_state TEXT NOT NULL DEFAULT 'legacy' CHECK (source_state IN ('queued','running','waiting_user','completed','blocked','cancelled','interrupted','legacy'));
ALTER TABLE plugin_build_events ADD COLUMN kind TEXT NOT NULL DEFAULT 'legacy' CHECK (kind IN ('status','progress','retry','legacy'));

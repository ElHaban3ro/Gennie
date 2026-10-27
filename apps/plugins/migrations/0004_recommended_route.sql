ALTER TABLE plugin_build_jobs ADD COLUMN recommended_route TEXT CHECK(recommended_route IS NULL OR recommended_route IN ('http','web','computer_use'));

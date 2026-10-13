CREATE TABLE decision_preferences (
    id INTEGER PRIMARY KEY CHECK(id=1),
    preload INTEGER NOT NULL DEFAULT 1 CHECK(preload IN (0,1)),
    unload_seconds INTEGER DEFAULT 600 CHECK(unload_seconds IS NULL OR (unload_seconds BETWEEN 60 AND 7200 AND unload_seconds % 60=0)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO decision_preferences(id) VALUES (1);

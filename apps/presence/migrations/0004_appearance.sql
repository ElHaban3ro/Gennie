CREATE TABLE presence_appearance (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    blur_radius INTEGER NOT NULL DEFAULT 6 CHECK (blur_radius BETWEEN 0 AND 30),
    saturation_percent INTEGER NOT NULL DEFAULT 100 CHECK (saturation_percent BETWEEN 0 AND 200),
    brightness_percent INTEGER NOT NULL DEFAULT 100 CHECK (brightness_percent BETWEEN 50 AND 150),
    tint_percent INTEGER NOT NULL DEFAULT 24 CHECK (tint_percent BETWEEN 0 AND 60),
    border_percent INTEGER NOT NULL DEFAULT 33 CHECK (border_percent BETWEEN 0 AND 100),
    border_width_hundredths INTEGER NOT NULL DEFAULT 75 CHECK (border_width_hundredths BETWEEN 0 AND 300),
    transition_ms INTEGER NOT NULL DEFAULT 120 CHECK (transition_ms BETWEEN 0 AND 300),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO presence_appearance (id) VALUES (1);

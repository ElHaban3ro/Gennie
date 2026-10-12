CREATE TABLE modules (
    name TEXT PRIMARY KEY,
    active INTEGER NOT NULL CHECK (active IN (0, 1))
);

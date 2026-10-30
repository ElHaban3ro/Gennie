-- Copia normalizada local; nunca una ruta externa ni metadatos del archivo elegido.
ALTER TABLE presence_assistants ADD COLUMN avatar_png BLOB
    CHECK (avatar_png IS NULL OR (typeof(avatar_png)='blob' AND length(avatar_png) BETWEEN 1 AND 1048576));
ALTER TABLE presence_assistants ADD COLUMN avatar_revision INTEGER NOT NULL DEFAULT 0
    CHECK (avatar_revision >= 0);

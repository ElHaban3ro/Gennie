-- Cierre explícito sin afirmar que el efecto ocurrió o dejó de ocurrir.
ALTER TABLE browser_web_attempts ADD COLUMN closed_unconfirmed_at TEXT;

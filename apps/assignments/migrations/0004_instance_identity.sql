ALTER TABLE assignments ADD COLUMN instance_id TEXT;
UPDATE assignments SET instance_id = lower(hex(randomblob(16)));
CREATE UNIQUE INDEX assignments_instance_id ON assignments(instance_id);
CREATE TRIGGER assignments_instance_insert BEFORE INSERT ON assignments
WHEN NEW.instance_id IS NULL OR length(NEW.instance_id) != 32 OR NEW.instance_id GLOB '*[^0-9a-f]*'
BEGIN SELECT RAISE(ABORT, 'Identidad de encargo inválida'); END;
CREATE TRIGGER assignments_instance_immutable BEFORE UPDATE OF instance_id ON assignments
WHEN NEW.instance_id IS NOT OLD.instance_id
BEGIN SELECT RAISE(ABORT, 'La identidad del encargo es inmutable'); END;

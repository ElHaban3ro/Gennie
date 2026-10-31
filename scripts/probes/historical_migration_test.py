"""Regresiones del aislamiento y comparación; no abre datos personales."""
import hashlib
from pathlib import Path
import sqlite3
import tempfile
import unittest
from contextlib import closing
import historical_migration as audit


class AuditTests(unittest.TestCase):
    def test_source_is_read_only_closed_and_never_created_if_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'origen # propio.sqlite3'
            with closing(sqlite3.connect(path)) as source:
                source.execute('CREATE TABLE messages(id INTEGER PRIMARY KEY, text TEXT)')
                source.execute("INSERT INTO messages VALUES(1,'Dato ficticio')")
                source.commit()
            before = hashlib.sha256(path.read_bytes()).digest()
            with audit.read_only(path) as source:
                self.assertEqual(source.execute('PRAGMA query_only').fetchone()[0], 1)
                with self.assertRaises(sqlite3.OperationalError):
                    source.execute("UPDATE messages SET text='NO CAMBIAR'")
            with self.assertRaises(sqlite3.ProgrammingError):
                source.execute('SELECT 1')
            self.assertEqual(hashlib.sha256(path.read_bytes()).digest(), before)
            missing = Path(directory) / 'no-existe.sqlite3'
            with self.assertRaises(sqlite3.OperationalError), audit.read_only(missing):
                pass
            self.assertFalse(missing.exists())

    def test_all_original_values_are_checked_while_added_columns_are_explicit(self):
        with closing(sqlite3.connect(':memory:')) as connection:
            connection.execute('CREATE TABLE messages(id INTEGER PRIMARY KEY, text TEXT, data BLOB)')
            connection.execute('INSERT INTO messages VALUES(?,?,?)', (1,'Ejemplo sintético á 🐧',b'\x00\xff'))
            before = audit.inventory(connection)
            connection.execute('ALTER TABLE messages ADD COLUMN new_metadata TEXT')
            connection.execute("UPDATE messages SET new_metadata='aditivo'")
            self.assertEqual(audit.inventory(connection, before), before)
            connection.execute("UPDATE messages SET text='Contenido perdido'")
            self.assertNotEqual(audit.inventory(connection, before), before)
            connection.execute('ALTER TABLE messages RENAME COLUMN data TO missing_old_column')
            with self.assertRaises(RuntimeError):
                audit.inventory(connection, before)


if __name__ == '__main__':
    unittest.main()

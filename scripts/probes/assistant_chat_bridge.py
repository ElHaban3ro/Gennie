#!/usr/bin/env python3
"""Prepara datos ficticios históricos y prueba FRB real, sin cuentas ni inferencia."""
import hashlib
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FLUTTER = Path(os.environ.get('FLUTTER_ROOT', str(Path.home() / '.local/share/gennie-flutter'))) / 'bin/flutter'
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target/flutter'))).resolve()


def prepare_fixture(fixture):
    database = fixture / 'fixture.sqlite3'
    with sqlite3.connect(database) as connection:
        connection.executescript((ROOT / 'bootstrap/migrations/0001_modules.sql').read_text())
        connection.execute("CREATE TABLE _soc_migrations_bootstrap(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')))")
        connection.execute('INSERT INTO _soc_migrations_bootstrap VALUES(1,?,?)', (hashlib.sha256((ROOT / 'bootstrap/migrations/0001_modules.sql').read_bytes()).hexdigest(), '2026-10-28T00:00:00.000Z'))
        for name in ('transcription', 'decisions', 'llm', 'tts', 'reminders_plugin', 'reminders', 'presence', 'assignments', 'browser', 'plugins', 'proactivity'):
            connection.execute('INSERT INTO modules(name,active) VALUES(?,?)', (name, name == 'presence'))
        connection.execute("CREATE TABLE _soc_migrations_presence(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')))")
        for migration in sorted((ROOT / 'apps/presence/migrations').glob('*.sql')):
            version = int(migration.name.split('_')[0])
            if version > 11:
                continue
            content = migration.read_bytes()
            connection.executescript(content.decode())
            connection.execute('INSERT INTO _soc_migrations_presence VALUES(?,?,?)', (version, hashlib.sha256(content).hexdigest(), '2026-10-28T00:00:00.000Z'))
        for identifier in (1, 2):
            connection.execute("INSERT INTO presence_conversations(id,title,automatic_title,created_at,updated_at) VALUES(?,?,0,'2026-10-28T00:00:00.000Z','2026-10-28T00:00:00.000Z')", (identifier, f'Sesión {identifier}'))
        connection.execute('INSERT INTO presence_current_conversation VALUES(1,2)')
        for identifier in range(1, 76):
            connection.execute('INSERT INTO presence_history(id,conversation_id,speaker,text,created_at) VALUES(?,?,?,?,?)', (identifier, identifier % 2 + 1, 'user', f'Mensaje de ejemplo {identifier} · verificación', '2026-10-28T00:00:00.000Z'))
    # La migración y escritura de la cita usan el repositorio Rust real.
    subprocess.run(['cargo','run','--quiet','--locked','-p','gennie-presence','--example','assistant_reply_fixture','--',str(database)],cwd=ROOT,check=True,timeout=60)
    env_file = fixture / 'fixture.env'
    env_file.write_text(f'GENNIE_DATABASE_PATH={database}\n')
    return database, env_file


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --generate --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-assistant-bridge-') as directory:
        fixture = Path(directory)
        database, env_file = prepare_fixture(fixture)
        child_env = os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH', None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY', None)
        subprocess.run([
            str(FLUTTER), 'test', 'test/assistant_chat_bridge_test.dart',
            f'--dart-define=GENNIE_BRIDGE_SMOKE_LIBRARY={library}',
            f'--dart-define=GENNIE_BRIDGE_SMOKE_HELPER={helper}',
            f'--dart-define=GENNIE_BRIDGE_SMOKE_ENV={env_file}',
        ], cwd=ROOT / 'frontend', env=child_env, check=True, timeout=60)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0] == 40
            assert connection.execute('SELECT source_message_id,text FROM presence_message_replies WHERE message_id=76').fetchone() == (None, 'Mensaje de ejemplo 2 · verificación')
            assert connection.execute('SELECT count(*) FROM presence_assistant_conversations').fetchone()[0] == 2
            assert connection.execute('SELECT name FROM presence_assistants WHERE id="gennie"').fetchone()[0] == 'Mi Gennie · personal'
            assert connection.execute('SELECT conversation_id FROM presence_current_conversation').fetchone()[0] == 2
            expected_version = max(int(path.name.split('_')[0]) for path in (ROOT / 'apps/presence/migrations').glob('*.sql'))
            assert connection.execute('SELECT max(version) FROM _soc_migrations_presence').fetchone()[0] == expected_version
            assert connection.execute('SELECT message_id,outcome FROM presence_tool_uses ORDER BY id').fetchall() == [(77,'completed'),(77,'failed'),(77,'unknown')]
            assert connection.execute('SELECT avatar_revision,length(avatar_png)>0 FROM presence_assistants WHERE id="gennie"').fetchone() == (1, 1)
    print('FRB y SQLite comprobados; fixture temporal cerrado y retirado.')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Aviso del bot por Flutter/FRB/SQLite reales, sin modelos ni cuentas."""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FLUTTER = Path(os.environ.get('FLUTTER_ROOT', str(Path.home() / '.local/share/gennie-flutter'))) / 'bin/flutter'
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target/flutter'))).resolve()


def migrate(connection, namespace, directory):
    table = f'_soc_migrations_{namespace}'
    connection.execute(f'CREATE TABLE {table}(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL)')
    for migration in sorted(directory.glob('*.sql')):
        connection.executescript(migration.read_text())
        connection.execute(f'INSERT INTO {table} VALUES(?,?,?)', (
            int(migration.name.split('_')[0]), hashlib.sha256(migration.read_bytes()).hexdigest(), '2026-10-29T00:00:00.000Z'))
    connection.commit()


def prepare_fixture(directory):
    database = directory / 'fixture.sqlite3'
    with sqlite3.connect(database) as connection:
        migrate(connection, 'bootstrap', ROOT / 'bootstrap/migrations')
        for name in ('transcription','decisions','llm','tts','reminders_plugin','reminders','presence','assignments','browser','plugins','proactivity'):
            connection.execute('INSERT INTO modules(name,active) VALUES(?,?)', (name, name in ('presence','plugins')))
        connection.commit()
        migrate(connection, 'presence', ROOT / 'apps/presence/migrations')
        migrate(connection, 'plugins', ROOT / 'apps/plugins/migrations')
        for identifier in (1, 2):
            date = f'2026-10-29T10:0{identifier}:00.000Z'
            connection.execute('INSERT INTO presence_conversations(id,title,created_at,updated_at) VALUES(?,?,?,?)', (identifier, f'Sesión ficticia {identifier}', date, date))
            connection.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(?,'user',?,?)", (identifier, f'Consulta ficticia {identifier}', date))
        connection.execute('INSERT INTO presence_current_conversation VALUES(1,2)')
        connection.execute("INSERT INTO presence_history(id,conversation_id,speaker,text,created_at) VALUES(3,1,'assistant','La preparación se interrumpió. Puedes revisar el trabajo y continuar cuando quieras.','2026-10-29T10:05:00.000Z')")
        origin = json.dumps({'conversation_id': 1, 'bot_id': 'gennie', 'epoch': 1, 'provider_thread': None})
        connection.execute("INSERT INTO plugin_build_jobs(id,request_key,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,max_attempts,max_seconds,summary,delegation_origin,legacy_origin) VALUES(1,'bridge-notice','fixture_notice','https://invalid.example','Preparación ficticia',1,0,'interrupted',1,30,'PRIVATE_HELPER_TEXT',?,0)", (origin,))
        connection.execute("INSERT INTO plugin_build_events(id,job_id,message,delivered,source_state,kind) VALUES(1,1,'PRIVATE_HELPER_TEXT',1,'interrupted','status')")
        connection.execute("INSERT INTO presence_delegation_messages VALUES(3,'plugin-build:1','plugin-build:1','plugin_builder',1,1,'fixture_notice')")
    env = directory / 'fixture.env'
    env.write_text(f'GENNIE_DATABASE_PATH={database}\n')
    return database, env


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-bot-notice-bridge-') as directory:
        database, env = prepare_fixture(Path(directory))
        child_env = os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH', None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY', None)
        subprocess.run([
            str(FLUTTER), 'test', 'test/bot_notice_bridge_test.dart',
            f'--dart-define=GENNIE_BOT_BRIDGE_LIBRARY={library}',
            f'--dart-define=GENNIE_BOT_BRIDGE_HELPER={helper}',
            f'--dart-define=GENNIE_BOT_BRIDGE_ENV={env}',
        ], cwd=ROOT/'frontend', env=child_env, check=True, timeout=90)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0] == 3
            assert connection.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0] == 1
            assert connection.execute('SELECT state FROM plugin_build_jobs').fetchone()[0] == 'interrupted'
            assert connection.execute('SELECT count(*) FROM codex_conversation').fetchone()[0] == 0
            assert connection.execute('SELECT count(*) FROM plugin_mcp_attempts').fetchone()[0] == 0
            assert connection.execute('SELECT conversation_id FROM presence_current_conversation').fetchone()[0] == 2
    print('Cuatro avisos Flutter/FRB/SQLite correctos; sin ejecución, modelos ni cuentas. Fixture cerrado y retirado.')


if __name__ == '__main__':
    main()

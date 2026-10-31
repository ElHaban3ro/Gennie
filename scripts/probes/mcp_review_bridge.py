#!/usr/bin/env python3
"""Revisión MCP por Flutter/FRB real, con SQLite ficticio y sin llamadas externas."""
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


def prepare_fixture(directory):
    database = directory / 'fixture.sqlite3'
    with sqlite3.connect(database) as connection:
        migration = ROOT / 'bootstrap/migrations/0001_modules.sql'
        connection.executescript(migration.read_text())
        connection.execute("CREATE TABLE _soc_migrations_bootstrap(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL)")
        connection.execute('INSERT INTO _soc_migrations_bootstrap VALUES(1,?,?)', (hashlib.sha256(migration.read_bytes()).hexdigest(), '2026-10-29T00:00:00.000Z'))
        for name in ('transcription','decisions','llm','tts','reminders_plugin','reminders','presence','assignments','browser','plugins','proactivity'):
            connection.execute('INSERT INTO modules(name,active) VALUES(?,?)', (name, name in ('presence','plugins')))
        connection.execute("CREATE TABLE _soc_migrations_plugins(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_at TEXT NOT NULL)")
        for migration in sorted((ROOT / 'apps/plugins/migrations').glob('*.sql')):
            connection.executescript(migration.read_text())
            connection.execute('INSERT INTO _soc_migrations_plugins VALUES(?,?,?)', (int(migration.name.split('_')[0]), hashlib.sha256(migration.read_bytes()).hexdigest(), '2026-10-29T00:00:00.000Z'))
        for number in range(1,5):
            identifier = f'fixture_{number}'
            name = f'MCP real de prueba {number}'
            specification = {'id':identifier,'name':name,'connection':{'type':'http','url':'https://invalid.example/mcp','bearer_token_env_var':None},'timeout_seconds':60}
            tools = {'echo':{'name':'echo','description':'Herramienta ficticia que no se ejecuta en esta prueba.','inputSchema':{'type':'object','properties':{'message':{'type':'string','title':'Mensaje'}},'required':['message'],'additionalProperties':False}}}
            connection.execute('INSERT INTO plugin_mcp_sources(id,document,enabled,tools,allowed_tools) VALUES(?,?,1,?,?)', (identifier,json.dumps(specification),json.dumps(tools),json.dumps(['echo'])))
            connection.execute('INSERT INTO plugin_mcp_attempts(source_id,source_revision,server_name,tool) VALUES(?,1,?,?)',(identifier,name,'echo'))
    env = directory / 'fixture.env'
    env.write_text(f'GENNIE_DATABASE_PATH={database}\n')
    return database, env


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-mcp-review-bridge-') as directory:
        database, env = prepare_fixture(Path(directory))
        child_env = os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH', None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY', None)
        completed = subprocess.run([
            str(FLUTTER),'test','test/mcp_review_bridge_test.dart',
            f'--dart-define=GENNIE_MCP_BRIDGE_LIBRARY={library}',
            f'--dart-define=GENNIE_MCP_BRIDGE_HELPER={helper}',
            f'--dart-define=GENNIE_MCP_BRIDGE_ENV={env}',
        ],cwd=ROOT/'frontend',env=child_env,check=False,timeout=90)
        if completed.returncode:
            with sqlite3.connect(database) as failed:
                print('ESTADO DE FIXTURE TRAS FALLO:',failed.execute('SELECT state,count(*) FROM plugin_mcp_attempts GROUP BY state').fetchall(),flush=True)
            completed.check_returncode()
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT count(*) FROM plugin_mcp_attempts').fetchone()[0] == 4
            assert connection.execute("SELECT count(*) FROM plugin_mcp_attempts WHERE state='reviewed' AND resolution='not_applied' AND reason='interrupted'").fetchone()[0] == 4
            assert connection.execute('SELECT min(revision),max(revision) FROM plugin_mcp_sources').fetchone() == (2,2)
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0] == 0
            assert connection.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0] == 0
            assert connection.execute('SELECT max(version) FROM _soc_migrations_plugins').fetchone()[0] == 8
    print('Cuatro flujos Flutter/FRB/SQLite correctos; sin otra llamada, modelos ni cuentas. Fixture cerrado y retirado.')


if __name__ == '__main__':
    main()

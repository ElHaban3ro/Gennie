#!/usr/bin/env python3
"""Conexión y prueba manual Flutter/FRB/MCP reales, sin cuenta ni inferencia."""
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET, migrate


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila con scripts/dev.sh --build-only antes de la sonda.')
    with tempfile.TemporaryDirectory(prefix='gennie-mcp-lifecycle-') as temporary:
        directory = Path(temporary)
        database = directory / 'fixture.sqlite3'
        with sqlite3.connect(database) as connection:
            migrate(connection, 'bootstrap', ROOT / 'bootstrap/migrations')
            for name in ('transcription','decisions','llm','tts','reminders_plugin','reminders','presence','assignments','browser','plugins','proactivity'):
                connection.execute('INSERT INTO modules(name,active) VALUES(?,?)', (name, name in ('presence','plugins')))
            connection.commit()
            migrate(connection, 'presence', ROOT / 'apps/presence/migrations')
            migrate(connection, 'plugins', ROOT / 'apps/plugins/migrations')
        env_file = directory / 'fixture.env'
        env_file.write_text(f'GENNIE_DATABASE_PATH={database}\n')
        calls = directory / 'calls.jsonl'
        env = os.environ.copy()
        env.pop('GENNIE_DATABASE_PATH', None)
        env.pop('GENNIE_OPENROUTER_API_KEY', None)
        clients = Path.home() / 'Library/Application Support/Gennie/plugins/mcp'
        prior_clients = {p.name for p in clients.iterdir()} if clients.is_dir() else set()
        try:
            subprocess.run([str(FLUTTER),'test','test/mcp_lifecycle_bridge_test.dart',
                        f'--dart-define=GENNIE_MCP_LIFECYCLE_LIBRARY={library}',
                        f'--dart-define=GENNIE_MCP_LIFECYCLE_HELPER={helper}',
                        f'--dart-define=GENNIE_MCP_LIFECYCLE_ENV={env_file}',
                        f'--dart-define=GENNIE_MCP_LIFECYCLE_PROGRAM={sys.executable}',
                        f'--dart-define=GENNIE_MCP_LIFECYCLE_ARGS={json.dumps([str(ROOT/"scripts/probes/mcp_stdio_fixture.py"),str(calls)])}',
                        f'--dart-define=GENNIE_MCP_LIFECYCLE_CALLS={calls}'],
                           cwd=ROOT/'frontend',env=env,check=True,timeout=60)
        finally:
            # Retirar solo los perfiles de IDs recién creados en la DB propia.
            with sqlite3.connect(database) as connection:
                for (identifier,) in connection.execute('SELECT id FROM plugin_mcp_sources'):
                    if identifier not in prior_clients and re.fullmatch(r'mcp_[0-9]+', identifier):
                        owned = clients / identifier
                        if owned.exists():
                            shutil.rmtree(owned)
                        assert not owned.exists()
        assert len(calls.read_text().splitlines()) == 2
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT state,resolution FROM plugin_mcp_attempts ORDER BY id').fetchall() == [('completed',None),('reviewed','applied')]
            assert connection.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0] == 0
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0] == 0
            assert connection.execute('SELECT count(*) FROM codex_conversation').fetchone()[0] == 0
            assert 'synthetic_payload' not in database.read_bytes().decode(errors='ignore')
    print('Flutter/FRB/MCP: conexión, permisos, dos envíos, bloqueo, revisión humana y reapertura; sin modelos/cuentas. Fixture retirado.')


if __name__ == '__main__':
    main()

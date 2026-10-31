#!/usr/bin/env python3
"""Archivo y tres páginas de revisiones por encargo sobre FRB real, sin modelos."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from proposals_bridge import ROOT, FLUTTER, TARGET, fixture as base_fixture


def fixture(directory):
    database, env = base_fixture(directory)
    with sqlite3.connect(database) as connection:
        connection.execute("UPDATE assignment_reviews SET created_at='2026-10-29T13:00:00Z'")
        for number in range(1, 5):
            source = connection.execute('SELECT scope,submission FROM assignment_reviews WHERE id=?', (f'review-{number}',)).fetchone()
            for index in range(23):
                identifier = f'history-{number}-{index:02d}'
                candidates = json.loads(source[1])
                candidates[0]['title'] = f'Historial real {number} {index:02d}'
                connection.execute("INSERT INTO assignment_reviews(id,request_key,request,scope,submission,state,created_at) VALUES(?,?,'{}',?,?,'ready','2026-10-29T13:00:00Z')", (identifier, identifier, source[0], json.dumps(candidates)))
                connection.execute('INSERT INTO assignment_proposals(review_id,ordinal,candidate) VALUES(?,0,?)', (identifier, json.dumps(candidates[0])))
    return database, env


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-proposal-history-') as directory:
        database, env = fixture(Path(directory))
        child_env = os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH', None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY', None)
        subprocess.run([str(FLUTTER), 'test', 'test/proposal_history_bridge_test.dart', f'--dart-define=GENNIE_PROPOSAL_HISTORY_LIBRARY={library}', f'--dart-define=GENNIE_PROPOSAL_HISTORY_HELPER={helper}', f'--dart-define=GENNIE_PROPOSAL_HISTORY_ENV={env}'], cwd=ROOT/'frontend', env=child_env, check=True, timeout=120)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT min(revision),max(revision) FROM assignments').fetchone() == (1, 1)
            assert connection.execute('SELECT count(*) FROM assignment_review_archives').fetchone()[0] == 4
            assert connection.execute('SELECT count(*) FROM assignment_reviews').fetchone()[0] == 96
            assert connection.execute("SELECT count(*) FROM assignment_proposals WHERE resolution='pending'").fetchone()[0] == 96
            assert all(json.loads(row[0])['description'] == 'Revisar novedades' for row in connection.execute('SELECT document FROM assignments'))
            for table in ('presence_history','codex_conversation','plugin_build_jobs','plugin_mcp_attempts','assignment_execution_runs'):
                assert connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0] == 0, table
    print('Cuatro recorridos Flutter/FRB: 24 revisiones por encargo, archivo explícito y consulta bloqueada para escritura; 12 capturas. 96 borradores y planes intactos. Sin modelos/cuentas/efectos; fixture cerrado y retirado.')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Revisar solicitudes del chat sobre Flutter/FRB/SQLite, sin inferencia."""
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
        connection.execute("UPDATE presence_conversations SET provider='codex',provider_thread_id='chat-review-fixture' WHERE id=1")
        for number in range(1, 5):
            root = f'review-{number}'
            text = f'Genera propuestas para Resumen real {number}, conservando las fuentes y permisos actuales.'
            at = f'2026-10-29T13:00:0{number}Z'
            message = connection.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(1,'user',?,?)", (text, at)).lastrowid
            scope = json.loads(connection.execute('SELECT scope FROM assignment_reviews WHERE id=?', (root,)).fetchone()[0])
            scope['instructions'] = text
            key = f'chat:1:1:{message}'
            request = {'request_key':key, 'assignments':[{k:scope['assignments'][0][k] for k in ('id','instance_id','revision')}], 'origin':scope['origin'], 'instructions':text, 'use_guide':False}
            connection.execute('UPDATE assignment_reviews SET request_key=?,request=?,scope=? WHERE id=?', (key, json.dumps(request), json.dumps(scope), root))
            connection.execute('INSERT INTO assignment_chat_review_requests VALUES(?,1,1,?,?,0,?)', (root, message, text, at))
            connection.execute('INSERT INTO assignment_review_inputs VALUES(?,?,?,0,?)', (root, message, text, at))
    return database, env


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-chat-review-') as directory:
        database, env = fixture(Path(directory))
        child_env = os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH', None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY', None)
        subprocess.run([str(FLUTTER), 'test', 'test/proposals_bridge_test.dart', '--dart-define=GENNIE_PROPOSALS_FROM_CHAT=true', f'--dart-define=GENNIE_PROPOSALS_BRIDGE_LIBRARY={library}', f'--dart-define=GENNIE_PROPOSALS_BRIDGE_HELPER={helper}', f'--dart-define=GENNIE_PROPOSALS_BRIDGE_ENV={env}'], cwd=ROOT/'frontend', env=child_env, check=True, timeout=90)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT min(revision),max(revision) FROM assignments').fetchone() == (2, 2)
            assert connection.execute("SELECT count(*) FROM assignment_proposals WHERE resolution='applied' AND applied_revision=2").fetchone()[0] == 4
            assert connection.execute('SELECT count(*) FROM assignment_chat_review_requests').fetchone()[0] == 4
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0] == 4
            assert all(json.loads(row[0])['use_guide'] is False for row in connection.execute('SELECT scope FROM assignment_reviews'))
            for table in ('codex_conversation','plugin_build_jobs','plugin_mcp_attempts','assignment_execution_runs'):
                assert connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0] == 0, table
    print('Cuatro revisiones del chat Flutter/FRB: fuente literal conservada, borradores consultados y cuatro aplicaciones humanas; fixture cerrado y retirado. Sin inferencia, cuentas ni efectos externos.')


if __name__ == '__main__':
    main()

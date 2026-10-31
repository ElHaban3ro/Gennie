#!/usr/bin/env python3
"""Consulta y aplicación de versiones sobre Flutter/FRB/SQLite, sin modelos."""
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
        connection.execute("UPDATE presence_conversations SET provider='codex',provider_thread_id='probe-origin' WHERE id=1")
        for number in range(1, 5):
            root = f'review-{number}'
            child = f'continued-{number}'
            source = connection.execute('SELECT scope,submission FROM assignment_reviews WHERE id=?', (root,)).fetchone()
            scope, candidates = map(json.loads, source)
            initial = f'Revisa el resumen {number} y evita duplicados'
            response = f'Agrupa las novedades del resumen {number} en una sola entrega'
            at = f'2026-10-29T13:00:0{number}Z'
            initial_id = connection.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(1,'user',?,?)", (initial, at)).lastrowid
            reply_id = connection.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(1,'user',?,?)", (response, at)).lastrowid
            connection.execute('INSERT INTO assignment_review_inputs VALUES(?,?,?,?,?)', (root, initial_id, initial, False, at))
            scope.update(initial_instructions=scope['instructions'], editable_ids=[number], instructions=response)
            candidates[0].update(title=f'Propuesta acordada {number}', rationale='Incorporar la respuesta del usuario')
            candidates[0]['document']['description'] = 'Resumen ajustado con la respuesta'
            connection.execute("INSERT INTO assignment_reviews(id,request_key,request,scope,submission,state) VALUES(?,?,?,?,?,'ready')", (child, child, json.dumps({'parent_review_id':root}), json.dumps(scope), json.dumps(candidates)))
            connection.execute('INSERT INTO assignment_proposals(review_id,ordinal,candidate) VALUES(?,0,?)', (child, json.dumps(candidates[0])))
            connection.execute('INSERT INTO assignment_review_inputs VALUES(?,?,?,?,?)', (child, reply_id, response, False, at))
            connection.execute('INSERT INTO assignment_review_refinements VALUES(?,?,?,?,?,?,?,?)', (child, root, root, 2, reply_id, response, False, at))
    return database, env


def main():
    library = TARGET / 'debug/libgennie_backend.dylib'
    helper = TARGET / 'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-refinement-bridge-') as directory:
        database, env = fixture(Path(directory))
        child_env = os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH', None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY', None)
        subprocess.run([str(FLUTTER), 'test', 'test/proposal_refinement_bridge_test.dart', f'--dart-define=GENNIE_REFINEMENT_LIBRARY={library}', f'--dart-define=GENNIE_REFINEMENT_HELPER={helper}', f'--dart-define=GENNIE_REFINEMENT_ENV={env}'], cwd=ROOT/'frontend', env=child_env, check=True, timeout=90)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            assert connection.execute('SELECT min(revision),max(revision) FROM assignments').fetchone() == (2, 2)
            assert connection.execute("SELECT count(*) FROM assignment_proposals WHERE review_id LIKE 'continued-%' AND resolution='applied' AND applied_revision=2").fetchone()[0] == 4
            assert connection.execute("SELECT count(*) FROM assignment_proposals WHERE review_id LIKE 'review-%' AND resolution='pending'").fetchone()[0] == 4
            assert all(json.loads(row[0])['description'] == 'Resumen ajustado con la respuesta' for row in connection.execute('SELECT document FROM assignments'))
            assert all(json.loads(row[0])['document']['description'] == 'Agrupar solo los cambios nuevos' for row in connection.execute("SELECT candidate FROM assignment_proposals WHERE review_id LIKE 'review-%'"))
            for table, count in [('assignment_review_refinements', 4), ('assignment_review_inputs', 8), ('presence_history', 8), ('codex_conversation', 0), ('plugin_build_jobs', 0), ('assignment_execution_runs', 0)]:
                assert connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0] == count, table
    print('Cuatro recorridos Flutter/FRB: versión anterior bloqueada, posterior consultada y aplicada una vez; ocho capturas. Sin modelos, cuentas ni ejecución externa; fixture cerrado y retirado.')


if __name__ == '__main__':
    main()

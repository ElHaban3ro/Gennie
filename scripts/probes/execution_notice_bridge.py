#!/usr/bin/env python3
"""Avisos de ejecuciones ficticias en Flutter/FRB/SQLite; sin modelos ni efectos."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET
from execution_bridge import fixture as execution_fixture


def fixture(directory):
    database,env=execution_fixture(directory)
    with sqlite3.connect(database) as c:
        for identifier in (1,2):
            c.execute('INSERT INTO presence_conversations(id,title,created_at,updated_at) VALUES(?,?,?,?)',(identifier,f'Sesión ficticia {identifier}',f'2026-10-29T10:0{identifier}:00.000Z',f'2026-10-29T10:0{identifier}:00.000Z'))
            c.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(?,'user',?,?)",(identifier,f'Contexto ficticio {identifier}',f'2026-10-29T10:0{identifier}:00.000Z'))
        c.execute('INSERT INTO presence_current_conversation VALUES(1,2)')
        for number in range(1,5):
            c.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(1,'assistant',?,?)",(f'Resultado pendiente {number}: la función no confirmó el efecto. Compruébalo antes de reactivar.',f'2026-10-29T10:10:0{number}.000Z'))
            reference={'assignment_id':number,'instance_id':f'{number:032x}','policy_id':f'delivery-{number}','policy_revision':1,'run_id':number,'state':'uncertain'}
            message=c.execute('SELECT last_insert_rowid()').fetchone()[0]
            c.execute('INSERT INTO presence_execution_messages(message_id,event_key,reference) VALUES(?,?,?)',(message,f'assignment-execution:{number}',json.dumps(reference)))
            c.execute('INSERT INTO presence_provider_input_messages(message_id,conversation_id) VALUES(?,1)',(message,))
        c.execute("UPDATE presence_conversations SET updated_at='2026-10-29T10:10:04.000Z' WHERE id=1")
    return database,env


def main():
    library=TARGET/'debug/libgennie_backend.dylib'
    helper=TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-execution-notice-bridge-') as directory:
        database,env=fixture(Path(directory))
        child_env=os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH',None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY',None)
        subprocess.run([str(FLUTTER),'test','test/execution_notice_bridge_test.dart',f'--dart-define=GENNIE_EXECUTION_NOTICE_LIBRARY={library}',f'--dart-define=GENNIE_EXECUTION_NOTICE_HELPER={helper}',f'--dart-define=GENNIE_EXECUTION_NOTICE_ENV={env}'],cwd=ROOT/'frontend',env=child_env,check=True,timeout=90)
        with sqlite3.connect(database) as c:
            assert c.execute('PRAGMA foreign_key_check').fetchall()==[]
            assert c.execute("SELECT count(*) FROM assignment_execution_runs WHERE resolution='not_applied'").fetchone()[0]==4
            assert c.execute('SELECT count(*) FROM presence_execution_messages').fetchone()[0]==4
            assert c.execute('SELECT count(*) FROM presence_history').fetchone()[0]==6
            assert c.execute('SELECT count(*) FROM presence_provider_input_messages WHERE user_message_id IS NULL').fetchone()[0]==4
            assert c.execute('SELECT count(*) FROM presence_provider_inputs').fetchone()[0]==0
            assert c.execute('SELECT conversation_id FROM presence_current_conversation').fetchone()[0]==2
            assert c.execute('SELECT min(revision),max(revision) FROM assignments').fetchone()==(1,1)
            assert c.execute('SELECT count(*) FROM codex_conversation').fetchone()[0]==0
            assert c.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0]==0
            assert c.execute('SELECT count(*) FROM plugin_mcp_attempts').fetchone()[0]==0
    print('Cuatro avisos tipados y comprobaciones FRB/SQLite correctos; sesión activa intacta, cero modelos o efectos. Fixture cerrado y retirado.')


if __name__=='__main__':
    main()

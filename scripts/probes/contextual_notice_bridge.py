#!/usr/bin/env python3
"""Estado y revocación de avisos contextuales con Flutter/FRB/SQLite propios."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET, migrate
from execution_bridge import fixture as execution_fixture


def fixture(directory):
    database, env = execution_fixture(directory)
    with sqlite3.connect(database) as c:
        migrate(c, 'llm', ROOT/'apps/llm/migrations')
        c.execute("INSERT INTO presence_conversations(id,provider,provider_thread_id,title) VALUES(1,'codex','origin','Origen ficticio')")
        for number in range(1, 5):
            document = json.loads(c.execute('SELECT document FROM assignments WHERE id=?', (number,)).fetchone()[0])
            document['follow_up'] = {'scope':'Novedades de prueba','signals':[], 'decision_guidance':'Evitar avisos repetidos','action_scope':None}
            c.execute('UPDATE assignments SET document=? WHERE id=?', (json.dumps(document),number))
            pid = f'preparation-{number}'
            policy = json.loads(c.execute('SELECT document FROM assignment_execution_versions WHERE policy_id=?', (pid,)).fetchone()[0])
            destination = {'conversation_id':1,'bot_id':'gennie','epoch':1,'provider_thread':'origin'}
            policy.update(contextual_notice=True,delivery=destination)
            policy['assignment']['document'] = document
            c.execute('UPDATE assignment_execution_versions SET document=? WHERE policy_id=?', (json.dumps(policy),pid))
            c.execute("UPDATE assignment_execution_policies SET enabled=0,state='paused',next_at=NULL WHERE id=?",(pid,))
            c.execute("UPDATE assignment_execution_runs SET policy_id=?,state='succeeded',code='reported_success',output=? WHERE id=?", (pid,json.dumps({'changes':['Resultado ficticio conservado']}),number))
            reference={'assignment_id':number,'instance_id':f'{number:032x}','policy_id':pid,'policy_revision':1,'run_id':number,'state':'succeeded'}
            update={'event_key':f'assignment-execution:{number}','origin':destination,'reference':reference,'title':document['title'],'scheduled_at':'2026-10-29T13:00:00Z','state':'succeeded','code':'reported_success','observation':'Resultado ficticio conservado','observation_truncated':False,'guidance':{'objective':document['description'],'scope':'Novedades de prueba','signals':[],'decision_guidance':'Evitar avisos repetidos'}}
            c.execute("INSERT INTO llm_execution_notices(event_key,bot_id,document,state,guide_state,notice_action,notice_reason,failure_code) VALUES(?,'gennie',?,'discarded','unavailable','record','Valoración no disponible; resultado conservado','render_failed')", (update['event_key'],json.dumps(update)))
    return database,env


def main():
    library=TARGET/'debug/libgennie_backend.dylib'; helper=TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-contextual-bridge-') as directory:
        database,env=fixture(Path(directory));child_env=os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH',None);child_env.pop('GENNIE_OPENROUTER_API_KEY',None)
        subprocess.run([str(FLUTTER),'test','test/contextual_notice_bridge_test.dart',f'--dart-define=GENNIE_CONTEXTUAL_LIBRARY={library}',f'--dart-define=GENNIE_CONTEXTUAL_HELPER={helper}',f'--dart-define=GENNIE_CONTEXTUAL_ENV={env}'],cwd=ROOT/'frontend',env=child_env,check=True,timeout=90)
        with sqlite3.connect(database) as c:
            assert c.execute('PRAGMA foreign_key_check').fetchall()==[]
            for number in range(1,5):
                row=c.execute('SELECT revision,enabled FROM assignment_execution_policies WHERE id=?',(f'preparation-{number}',)).fetchone()
                assert row==(2,0),row
                versions=c.execute('SELECT revision,document FROM assignment_execution_versions WHERE policy_id=? ORDER BY revision',(f'preparation-{number}',)).fetchall()
                assert len(versions)==2
                assert json.loads(versions[0][1])['contextual_notice'] is True
                assert json.loads(versions[1][1])['contextual_notice'] is False
                assert json.loads(versions[1][1]).get('delivery') is None
            assert c.execute("SELECT count(*) FROM assignment_execution_runs WHERE state='succeeded' AND output IS NOT NULL AND resolution IS NULL").fetchone()[0]==4
            assert c.execute("SELECT count(*) FROM llm_execution_notices WHERE notice_action='record' AND guide_attempted_at IS NULL AND render_attempted_at IS NULL").fetchone()[0]==4
            for table in ('presence_history','presence_execution_messages','codex_conversation','plugin_build_jobs','plugin_mcp_attempts'):
                assert c.execute(f'SELECT count(*) FROM {table}').fetchone()[0]==0,table
    print('Cuatro estados y revocaciones FRB/SQLite correctos; versiones/resultados conservados, cero modelos o efectos. Fixture cerrado y retirado.')


if __name__=='__main__':
    main()

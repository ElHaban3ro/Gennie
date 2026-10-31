#!/usr/bin/env python3
"""Revisión humana de propuestas sobre FRB/SQLite reales, sin inferencia."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET, migrate


def fixture(directory):
    database = directory / 'fixture.sqlite3'
    with sqlite3.connect(database) as connection:
        migrate(connection, 'bootstrap', ROOT / 'bootstrap/migrations')
        for name in ('transcription','decisions','llm','tts','reminders_plugin','reminders','presence','assignments','browser','plugins','proactivity'):
            connection.execute('INSERT INTO modules(name,active) VALUES(?,?)', (name, name in ('presence','assignments')))
        connection.commit()
        migrate(connection, 'presence', ROOT / 'apps/presence/migrations')
        migrate(connection, 'events', ROOT / 'apps/assignments/migrations')
        connection.execute("INSERT INTO presence_conversations(id,title) VALUES(1,'Sesión de prueba')")
        connection.execute('INSERT INTO presence_current_conversation VALUES(1,1)')
        for number in range(1,5):
            identity = f'{number:032x}'
            original = {'title':f'Resumen real {number}','description':'Revisar novedades','kind':'recurring','priority':'normal','status':'active','timezone':'America/Bogota','recurrence':{'frequency':'daily','local_time':'08:00','weekdays':[]},'preparation':{'steps':[]},'applications':[]}
            proposed = {**original, 'description':'Agrupar solo los cambios nuevos'}
            reference = {'id':number,'instance_id':identity,'revision':1}
            request = {'request_key':f'review-{number}','assignments':[reference],'origin':{'conversation_id':1,'epoch':1},'instructions':'Evitar duplicados','use_guide':False}
            scope = {'origin':request['origin'],'assignments':[{**reference,'document':original}],'instructions':request['instructions'],'use_guide':False}
            candidate = {'assignment_id':number,'title':f'Mejora real {number}','rationale':'Evitar interrupciones repetidas','expected_benefit':'Resumen más útil','related_ids':[],'questions':[],'document':proposed}
            connection.execute('INSERT INTO assignments(id,request_key,document,instance_id) VALUES(?,?,?,?)',(number,f'fixture-{number}',json.dumps(original),identity))
            connection.execute("INSERT INTO assignment_reviews(id,request_key,request,scope,submission,state) VALUES(?,?,?,?,?,'ready')",(f'review-{number}',f'review-{number}',json.dumps(request),json.dumps(scope),json.dumps([candidate])))
            connection.execute('INSERT INTO assignment_proposals(review_id,ordinal,candidate) VALUES(?,0,?)',(f'review-{number}',json.dumps(candidate)))
    env = directory / 'fixture.env'
    env.write_text(f'GENNIE_DATABASE_PATH={database}\n')
    return database,env


def main():
    library = TARGET/'debug/libgennie_backend.dylib'
    helper = TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila primero con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-proposals-bridge-') as directory:
        database,env = fixture(Path(directory))
        child_env=os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH',None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY',None)
        subprocess.run([str(FLUTTER),'test','test/proposals_bridge_test.dart',f'--dart-define=GENNIE_PROPOSALS_BRIDGE_LIBRARY={library}',f'--dart-define=GENNIE_PROPOSALS_BRIDGE_HELPER={helper}',f'--dart-define=GENNIE_PROPOSALS_BRIDGE_ENV={env}'],cwd=ROOT/'frontend',env=child_env,check=True,timeout=90)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall()==[]
            assert connection.execute('SELECT min(revision),max(revision) FROM assignments').fetchone()==(2,2)
            assert connection.execute("SELECT count(*) FROM assignment_proposals WHERE resolution='applied' AND applied_revision=2").fetchone()[0]==4
            assert all(json.loads(row[0])['description']=='Agrupar solo los cambios nuevos' for row in connection.execute('SELECT document FROM assignments'))
            assert connection.execute('SELECT count(*) FROM codex_conversation').fetchone()[0]==0
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0]==0
            assert connection.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0]==0
    print('Cuatro revisiones humanas FRB/SQLite correctas; sin modelos, cuentas, navegador ni ejecución. Fixture cerrado y retirado.')


if __name__=='__main__':
    main()

#!/usr/bin/env python3
"""Revisión humana de ejecuciones ficticias con FRB/SQLite reales, sin efectos."""
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
        for number in range(1,5):
            identity = f'{number:032x}'
            document = {'title':f'Ejecución real {number}','description':'Consultar novedades ficticias','kind':'recurring','priority':'normal','status':'active','timezone':'America/Bogota','recurrence':{'frequency':'daily','local_time':'08:00','weekdays':[]},'preparation':{'interval_minutes':30,'rationale':'Revisión de prueba','steps':[]},'applications':[],'method':{'plugin_id':'fixture','functions':['read']},'authorization':{'scope':'Solo la fuente de prueba','allowed_routes':['http']}}
            connection.execute('INSERT INTO assignments(id,request_key,document,instance_id) VALUES(?,?,?,?)',(number,f'fixture-{number}',json.dumps(document),identity))
            for trigger in ('delivery','preparation'):
                policy_id=f'{trigger}-{number}'
                policy={'assignment':{'id':number,'instance_id':identity,'revision':1,'document':document},'trigger':trigger,'calendar':{'kind':'weekly','zone':'America/Bogota','local_time':'08:00','weekdays':[1,2,3,4,5,6,7],'dst':{'repeated':'earlier','missing':'skip'}},'target':{'kind':'web','plugin_id':'fixture','function':'read','revision':1},'arguments':{'query':f'Original {number}'},'route':'http','misfire':'skip_missed','max_lateness_minutes':5}
                if trigger == 'preparation':
                    policy['calendar']={'kind':'interval','anchor':1793278800,'seconds':1800}
                connection.execute('INSERT INTO assignment_execution_policies(id,assignment_id,instance_id,trigger,revision,enabled,state,next_at) VALUES(?,?,?,?,1,?,?,?)',(policy_id,number,identity,trigger,trigger=='preparation','active' if trigger=='preparation' else 'review_required','2099-01-01T13:00:00Z' if trigger=='preparation' else None))
                connection.execute('INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES(?,1,?)',(policy_id,json.dumps(policy)))
                if trigger=='delivery':
                    connection.execute("INSERT INTO assignment_execution_runs(id,policy_id,policy_revision,scheduled_at,state,code) VALUES(?,?,1,'2026-10-29T13:00:00Z','uncertain','effect_unconfirmed')",(number,policy_id))
    env = directory/'fixture.env'
    env.write_text(f'GENNIE_DATABASE_PATH={database}\n')
    return database,env


def main():
    library=TARGET/'debug/libgennie_backend.dylib'
    helper=TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila con ./scripts/dev.sh --build-only')
    with tempfile.TemporaryDirectory(prefix='gennie-execution-bridge-') as directory:
        database,env=fixture(Path(directory))
        child_env=os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH',None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY',None)
        subprocess.run([str(FLUTTER),'test','test/execution_bridge_test.dart',f'--dart-define=GENNIE_EXECUTION_BRIDGE_LIBRARY={library}',f'--dart-define=GENNIE_EXECUTION_BRIDGE_HELPER={helper}',f'--dart-define=GENNIE_EXECUTION_BRIDGE_ENV={env}'],cwd=ROOT/'frontend',env=child_env,check=True,timeout=90)
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA foreign_key_check').fetchall()==[]
            assert connection.execute('SELECT min(revision),max(revision) FROM assignments').fetchone()==(1,1)
            assert connection.execute("SELECT count(*) FROM assignment_execution_runs WHERE state='uncertain' AND resolution='not_applied'").fetchone()[0]==4
            assert connection.execute("SELECT count(*) FROM assignment_execution_policies WHERE enabled=0 AND state='paused'").fetchone()[0]==8
            assert connection.execute('SELECT count(*) FROM codex_conversation').fetchone()[0]==0
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0]==0
            assert connection.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0]==0
    print('Cuatro revisiones y cuatro pausas FRB/SQLite correctas; sin modelos, cuentas, navegador ni efectos. Fixture cerrado y retirado.')


if __name__=='__main__':
    main()

#!/usr/bin/env python3
"""Ficha de construcción sobre FRB/SQLite; mensajes del bot ficticios ya confirmados."""
from contextlib import closing
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET, prepare_fixture


def main():
    library=TARGET/'debug/libgennie_backend.dylib'
    helper=TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila con scripts/dev.sh --build-only antes de la sonda.')
    with tempfile.TemporaryDirectory(prefix='gennie-job-messages-') as temporary:
        database,env=prepare_fixture(Path(temporary))
        child_env=os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH',None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY',None)
        subprocess.run([str(FLUTTER),'test','test/job_messages_bridge_test.dart',
                        f'--dart-define=GENNIE_JOB_MESSAGES_LIBRARY={library}',
                        f'--dart-define=GENNIE_JOB_MESSAGES_HELPER={helper}',
                        f'--dart-define=GENNIE_JOB_MESSAGES_ENV={env}'],cwd=ROOT/'frontend',env=child_env,check=True,timeout=60)
        with closing(sqlite3.connect(database)) as connection:
            assert connection.execute('PRAGMA integrity_check').fetchone()[0]=='ok'
            assert connection.execute('PRAGMA foreign_key_check').fetchall()==[]
            assert connection.execute('SELECT count(*) FROM presence_history').fetchone()[0]==1
            assert connection.execute('SELECT conversation_id FROM presence_current_conversation').fetchone()[0]==2
            assert connection.execute('SELECT summary,state FROM plugin_build_jobs').fetchone()==('PRIVATE_HELPER_TEXT','interrupted')
            assert connection.execute('SELECT count(*) FROM plugin_build_events').fetchone()[0]==1
            for table in ('codex_conversation','plugin_mcp_attempts','assignment_execution_runs'):
                assert connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0]==0
    print('Cuatro fichas Flutter/FRB/SQLite: mensaje confirmado, helper no presentado, origen retirado sin fuga, selección conservada; fixture cerrado/retirado. Sin modelos, cuentas o efectos externos.')


if __name__=='__main__':
    main()

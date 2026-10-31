#!/usr/bin/env python3
"""Lee fragmentos sellados y comprueba cancelación tardía sin turno activo por FRB/SQLite."""
from contextlib import closing
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET, migrate


def main():
    library=TARGET/'debug/libgennie_backend.dylib'
    helper=TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila con scripts/dev.sh --build-only antes de la sonda.')
    for ending in ('cancelled','failed'):
        with tempfile.TemporaryDirectory(prefix='gennie-stop-bridge-') as directory:
            root=Path(directory);database=root/'fixture.sqlite3'
            with closing(sqlite3.connect(database)) as connection:
                migrate(connection,'bootstrap',ROOT/'bootstrap/migrations')
                for name in ('transcription','decisions','llm','tts','reminders_plugin','presence','assignments','browser','plugins','proactivity'):
                    connection.execute('INSERT INTO modules(name,active) VALUES(?,?)',(name,name=='presence'))
                connection.commit()
            subprocess.run(['cargo','run','--quiet','--locked','-p','gennie-presence','--example','interruption_fixture','--',str(database),ending],cwd=ROOT,check=True,timeout=60)
            env=root/'fixture.env';env.write_text(f'GENNIE_DATABASE_PATH={database}\n')
            child_env=os.environ.copy()
            for name in ('GENNIE_DATABASE_PATH','GENNIE_OPENROUTER_API_KEY','OPENROUTER_API_KEY','OPENAI_API_KEY','CODEX_API_KEY','ACCESS_TOKEN','CHATGPT_ACCESS_TOKEN'):
                child_env.pop(name,None)
            process=subprocess.Popen([str(FLUTTER),'test','test/chat_cancellation_bridge_test.dart',
                f'--dart-define=GENNIE_STOP_LIBRARY={library}',f'--dart-define=GENNIE_STOP_HELPER={helper}',
                f'--dart-define=GENNIE_STOP_ENV={env}',f'--dart-define=GENNIE_STOP_ENDING={ending}'],cwd=ROOT/'frontend',env=child_env,start_new_session=True)
            try:
                code=process.wait(timeout=60)
            except BaseException:
                try: os.killpg(process.pid,signal.SIGTERM)
                except ProcessLookupError: pass
                try: process.wait(timeout=3)
                finally:
                    try: os.killpg(process.pid,signal.SIGKILL)
                    except ProcessLookupError: pass
                    process.wait()
                raise
            if code: raise SystemExit(code)
            with closing(sqlite3.connect(database)) as connection:
                assert connection.execute('PRAGMA integrity_check').fetchone()[0]=='ok'
                assert connection.execute('PRAGMA foreign_key_check').fetchall()==[]
                assert connection.execute('SELECT speaker,interruption FROM presence_history ORDER BY id').fetchall()==[('user',None),('assistant',None),('assistant',ending),('system',None)]
                assert connection.execute('SELECT count(*) FROM codex_conversation').fetchone()[0]==0
                for table in ('plugin_build_jobs','plugin_mcp_attempts','assignment_execution_runs'):
                    assert connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0]==0
    print('FRB/SQLite: dos estados persistidos y cancelación tardía sin efectos. Sin cuenta, modelo o navegador; fixtures cerrados/retirados. No prueba cancelar un proveedor real desde la UI.')


if __name__=='__main__':
    main()

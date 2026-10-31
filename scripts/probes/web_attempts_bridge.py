#!/usr/bin/env python3
"""Panel humano real/FRB/SQLite con dos recibos ficticios; sin abrir catálogo o navegador."""
from contextlib import closing
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
from bot_notice_bridge import ROOT, FLUTTER, TARGET, migrate, prepare_fixture


def main():
    library=TARGET/'debug/libgennie_backend.dylib'
    helper=TARGET/'debug/gennie'
    if not library.is_file() or not helper.is_file():
        raise SystemExit('Compila con scripts/dev.sh --build-only antes de la sonda.')
    with tempfile.TemporaryDirectory(prefix='gennie-web-review-') as directory:
        database,env=prepare_fixture(Path(directory))
        with closing(sqlite3.connect(database)) as connection:
            migrate(connection,'browser',ROOT/'apps/browser/migrations')
            connection.execute("UPDATE modules SET active=1 WHERE name='browser'")
            for number in (1,2):
                connection.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation) VALUES('fixture_notice','Prueba web',?,5,'job_1','verify')",(f'function_{number}',))
            connection.commit()
        child_env=os.environ.copy()
        child_env.pop('GENNIE_DATABASE_PATH',None)
        child_env.pop('GENNIE_OPENROUTER_API_KEY',None)
        process=subprocess.Popen([str(FLUTTER),'test','test/web_attempts_bridge_test.dart',
                                  f'--dart-define=GENNIE_WEB_REVIEW_LIBRARY={library}',
                                  f'--dart-define=GENNIE_WEB_REVIEW_HELPER={helper}',
                                  f'--dart-define=GENNIE_WEB_REVIEW_ENV={env}'],cwd=ROOT/'frontend',env=child_env,start_new_session=True)
        try:
            code=process.wait(timeout=60)
        except BaseException:
            try:
                os.killpg(process.pid,signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=3)
            finally:
                try:
                    os.killpg(process.pid,signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
            raise
        if code:
            raise SystemExit(code)
        with closing(sqlite3.connect(database)) as connection:
            assert connection.execute('PRAGMA integrity_check').fetchone()[0]=='ok'
            assert connection.execute('PRAGMA foreign_key_check').fetchall()==[]
            assert connection.execute('SELECT state,resolution FROM browser_web_attempts ORDER BY id').fetchall()==[('reviewed','applied'),('reviewed','not_applied')]
            assert connection.execute('SELECT summary,state FROM plugin_build_jobs').fetchone()==('PRIVATE_HELPER_TEXT','interrupted')
            assert connection.execute('SELECT conversation_id FROM presence_current_conversation').fetchone()[0]==2
            for table in ('codex_conversation','plugin_mcp_attempts','assignment_execution_runs'):
                assert connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0]==0
    print('Dos revisiones del panel por FRB/SQLite correctas. Recibos y selección conservados; sin envío, modelos, cuentas, catálogo web o navegador. Fixture cerrado/retirado.')


if __name__=='__main__':
    main()

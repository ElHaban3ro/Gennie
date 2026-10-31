#!/usr/bin/env python3
"""Ensayo de migración sobre copia coherente; origen abierto con mode=ro."""
import hashlib
from contextlib import closing, contextmanager
import json
import os
from pathlib import Path
import selectors
import shlex
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote
import uuid

ROOT = Path(__file__).resolve().parents[2]


def source_path():
    value = os.environ.get('GENNIE_DATABASE_PATH')
    if value is None:
        for line in (ROOT / '.env').read_text().splitlines():
            key, sep, raw = line.partition('=')
            if sep and key.strip().removeprefix('export ') == 'GENNIE_DATABASE_PATH':
                parts = shlex.split(raw, comments=True)
                if len(parts) != 1 or '$' in parts[0]:
                    raise SystemExit('La ruta requiere resolución explícita; no se expanden expresiones.')
                value = parts[0]
    if not value:
        raise SystemExit('Falta GENNIE_DATABASE_PATH.')
    path = Path(value)
    return path if path.is_absolute() else ROOT / path


@contextmanager
def read_only(path):
    connection = sqlite3.connect('file:' + quote(str(path)) + '?mode=ro', uri=True, timeout=5)
    try:
        connection.execute('PRAGMA query_only=ON')
        yield connection
    finally:
        connection.close()


def quoted(name):
    return '"' + name.replace('"', '""') + '"'


def digest(rows):
    encoded = []
    for row in rows:
        values = [('blob', value.hex()) if isinstance(value, bytes) else (type(value).__name__, value) for value in row]
        encoded.append(json.dumps(values, ensure_ascii=False, separators=(',', ':')).encode())
    return hashlib.sha256(b'\n'.join(sorted(encoded))).hexdigest()


def inventory(connection, columns=None):
    tables = [row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
    result = {}
    for table in tables:
        names = [row[1] for row in connection.execute('PRAGMA table_info(' + quoted(table) + ')')]
        selected = columns[table]['columns'] if columns and table in columns else names
        if not set(selected) <= set(names):
            raise RuntimeError('Se retiraron columnas existentes de ' + table)
        rows = connection.execute('SELECT ' + ','.join(map(quoted, selected)) + ' FROM ' + quoted(table)).fetchall()
        result[table] = {'columns':selected, 'count':len(rows), 'digest':digest(rows)}
    return result


def interrupt_copy(path, migration):
    # El hijo pausa después del SQL/recibo reales y antes de COMMIT; solo se mata su PID.
    script = '''import hashlib,sqlite3,sys
from pathlib import Path
c=sqlite3.connect(sys.argv[1],isolation_level=None)
c.execute('PRAGMA foreign_keys=ON');c.execute('PRAGMA journal_mode=WAL');c.execute('PRAGMA synchronous=FULL')
c.execute('BEGIN IMMEDIATE')
sql=Path(sys.argv[2]).read_text();statement=''
for line in sql.splitlines(True):
 statement+=line
 if sqlite3.complete_statement(statement):c.execute(statement);statement=''
if statement.strip():raise RuntimeError('SQL incompleto')
c.execute("INSERT INTO _soc_migrations_presence(version,checksum) VALUES(12,?)",(hashlib.sha256(Path(sys.argv[2]).read_bytes()).hexdigest(),))
print('transaction_prepared',flush=True)
sys.stdin.readline()
raise RuntimeError('El lanzador debe interrumpir antes de commit')
'''
    child = subprocess.Popen([sys.executable, '-u', '-c', script, str(path), str(migration)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            if not selector.select(timeout=5) or child.stdout.readline().strip() != b'transaction_prepared':
                raise RuntimeError('No se alcanzó el punto de interrupción; no se informa rollback comprobado.')
        child.kill()
        child.wait(timeout=5)
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=5)
        for pipe in (child.stdin, child.stdout, child.stderr):
            pipe.close()


def main():
    output = ROOT / '.verification/migration-audit' / str(uuid.uuid4())
    output.mkdir(parents=True, mode=0o700)
    manifest = {'source_open_mode':'ro','source_sql_writes':False,'models_started':False,'raw_copies_retained':False}
    with tempfile.TemporaryDirectory(prefix='gennie-migration-copy-') as temporary:
        directory = Path(temporary)
        frozen = directory / 'original.sqlite3'
        frozen.touch(mode=0o600)
        start = time.monotonic()
        with read_only(source_path()) as source, closing(sqlite3.connect(frozen)) as destination:
            def progress(*_):
                if time.monotonic() - start > 5:
                    raise TimeoutError('Copia sin completar dentro de cinco segundos; origen intacto.')
            source.backup(destination, pages=64, progress=progress, sleep=.005)
        frozen.chmod(0o400)
        with read_only(frozen) as connection:
            original = inventory(connection)
            version = connection.execute('SELECT max(version) FROM _soc_migrations_presence').fetchone()[0]
            if version != 11:
                raise SystemExit('Este ensayo requiere una copia histórica de presence 0011; no se fuerza downgrade.')
            assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
        working = directory / 'audit.sqlite3'
        shutil.copyfile(frozen, working)
        working.chmod(0o600)
        interrupt_copy(working, ROOT / 'apps/presence/migrations/0012_assistant_identity.sql')
        with closing(sqlite3.connect(working)) as connection:
            assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert inventory(connection) == original
            assert connection.execute('SELECT max(version) FROM _soc_migrations_presence').fetchone()[0] == 11
            # Solo la copia se configura sin servicios antes de abrir el coordinador real.
            connection.execute('UPDATE modules SET active=0')
            for name in ('presence','assignments','llm','transcription','decisions','tts','reminders_plugin','browser','plugins','proactivity'):
                connection.execute('INSERT OR IGNORE INTO modules(name,active) VALUES(?,0)', (name,))
            connection.commit()
            baseline = inventory(connection)
            sequence_before = dict(connection.execute('SELECT name,seq FROM sqlite_sequence'))
        env = os.environ.copy()
        env.pop('GENNIE_DATABASE_PATH', None)
        env.pop('GENNIE_OPENROUTER_API_KEY', None)
        env['GENNIE_TEST_MIGRATION_COPY'] = str(working)
        with (output / 'cargo.log').open('x') as log:
            os.chmod(output / 'cargo.log', 0o600)
            result = subprocess.run(['cargo','test','--locked','-p','gennie','historical_copy_migrates_reopens_and_pages_without_effectful_modules','--','--ignored','--nocapture'],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,check=False,timeout=60)
        if result.returncode:
            raise SystemExit('Falló el ensayo; log privado: ' + str(output / 'cargo.log'))
        with closing(sqlite3.connect(working)) as connection, read_only(frozen) as before:
            after = inventory(connection, baseline)
            unchanged = []
            for table, record in baseline.items():
                if table.startswith('_soc_migrations_'):
                    columns = ','.join(map(quoted, record['columns']))
                    old = set(before.execute('SELECT ' + columns + ' FROM ' + quoted(table)))
                    new = set(connection.execute('SELECT ' + columns + ' FROM ' + quoted(table)))
                    assert old <= new, 'Historia aplicada alterada: ' + table
                else:
                    assert after.get(table) == record, 'Datos existentes alterados: ' + table
                unchanged.append(table)
            sequence_after = dict(connection.execute('SELECT name,seq FROM sqlite_sequence'))
            assert all(sequence_after.get(name, -1) >= value for name, value in sequence_before.items())
            assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert connection.execute('PRAGMA foreign_key_check').fetchall() == []
            legacy_jobs = connection.execute("SELECT count(*) FROM plugin_build_jobs WHERE legacy_origin=1 AND delegation_origin IS NULL").fetchone()[0]
            assert legacy_jobs == original['plugin_build_jobs']['count']
            assert connection.execute('SELECT count(*) FROM assignment_execution_policies').fetchone()[0] == 0
            profile_fields = ('model','effort','read_files','write_files','run_commands','local_actions','updated_at')
            for role in ('bot','helper'):
                fields = ','.join(map(quoted, profile_fields))
                assert connection.execute('SELECT ' + fields + ' FROM llm_architects WHERE role=?', (role,)).fetchone() == connection.execute('SELECT ' + fields + ' FROM llm_preferences WHERE id=1').fetchone()
            assert connection.execute("SELECT enabled FROM llm_architects WHERE role='guide'").fetchone()[0] == 0
            manifest.update(legacy_jobs_without_invented_origin=legacy_jobs, profiles_preserve_existing_bot_settings=True, execution_policies_created=0)
            manifest.update(existing_tables_preserved=unchanged, original_counts={name:original[name]['count'] for name in ('presence_conversations','presence_history','assignments','plugin_build_jobs')}, migration_versions={name:connection.execute('SELECT max(version) FROM ' + quoted(name)).fetchone()[0] for name in after if name.startswith('_soc_migrations_')})
        restored = directory / 'restore.sqlite3'
        shutil.copyfile(frozen, restored)
        with closing(sqlite3.connect(restored)) as connection:
            assert inventory(connection) == original
            assert connection.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
        manifest.update(rollback_after_process_kill_verified=True, backup_restore_verified=True, module_selection_changed_only_in_copy=True, **json.loads((directory / 'report.json').read_text()))
    manifest['raw_copies_removed'] = not directory.exists()
    with (output / 'report.json').open('x') as file:
        os.chmod(output / 'report.json', 0o600)
        json.dump(manifest, file, ensure_ascii=False, indent=2)
        file.write('\n')
    print(json.dumps({'report':str(output / 'report.json'),'counts':manifest['original_counts'],'pages':len(manifest['pages']),'visible_messages':manifest['visible_messages'],'first_measured_page':manifest['first_measured_page'],'active_modules_started':manifest['active_modules_started'],'raw_copies_removed':manifest['raw_copies_removed']},ensure_ascii=False))


if __name__ == '__main__':
    main()

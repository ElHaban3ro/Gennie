#!/usr/bin/env python3
"""Instancia release propia; interacción/Quit mediante Computer Use, sin modelos."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import selectors
import signal
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid
from proposals_bridge import ROOT, fixture


def qa_processes(app):
    """Solo procesos vivos del ejecutable exclusivo, nunca por nombre de app."""
    expected = os.path.realpath(app)
    result = subprocess.run(['ps', '-ax', '-o', 'pid=,stat=,comm='],
                            capture_output=True, text=True, check=True, timeout=5)
    matches = []
    for line in result.stdout.splitlines():
        parts = line.strip().split(None, 2)
        if (len(parts) == 3 and parts[0].isdigit() and not parts[1].startswith('Z')
                and os.path.realpath(parts[2]) == expected):
            matches.append(int(parts[0]))
    return sorted(matches)


def stop_qa_residuals(app):
    found = qa_processes(app)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for pid in qa_processes(app):
            if pid not in found:
                found.append(pid)
            # Revalidar ruta después del inventario antes de señalizar un PID.
            if pid in qa_processes(app):
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 2
        while qa_processes(app) and time.monotonic() < deadline:
            time.sleep(.05)
        if not qa_processes(app):
            break
    return {'found': sorted(found), 'remaining': qa_processes(app)}


def engine_errors(log):
    return [code for code, marker in (
        ('invalid_engine', 'Engine handle was invalid'),
        ('response_handle', 'Failed to create a FlutterPlatformMessageResponseHandle'),
        ('platform_send', 'Failed to send message to Flutter engine'),
    ) if marker in log]


def require_clean_quit(manifest, log):
    if (manifest['stop'] != 'already_exited' or manifest['exit_code'] != 0
            or manifest['integrity'] != 'ok' or manifest['foreign_keys']
            or manifest.get('qa_residual_processes', {}).get('found')
            or manifest.get('qa_residual_processes', {}).get('remaining')
            or engine_errors(log)):
        raise SystemExit('Fallo: no se verificó Salir con motor/SQLite sin errores observados.')


def profile_record(connection):
    identity, name, revision, image = connection.execute(
        'SELECT id,name,avatar_revision,avatar_png FROM presence_assistants WHERE id=?',
        ('gennie',),
    ).fetchone()
    return {'id': identity, 'name': name, 'avatar_revision': revision,
            'avatar_bytes': len(image) if image else 0,
            'avatar_sha256': hashlib.sha256(image).hexdigest() if image else None}


def quote_record(connection):
    return {
        'active_conversation': connection.execute('SELECT conversation_id FROM presence_current_conversation WHERE id=1').fetchone()[0],
        'sessions': connection.execute('SELECT id,provider_thread_id,deleted FROM presence_conversations ORDER BY id').fetchall(),
        'quotes': connection.execute('SELECT message_id,source_message_id,speaker,text,quoted_created_at,truncated FROM presence_message_replies ORDER BY message_id').fetchall(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--require-clean-quit', action='store_true',
                        help='Falla si no hubo salida propia 0 o se observa el error del canal al cerrar.')
    parser.add_argument('--profile-scenario', choices=('save', 'cancel', 'restore'),
                        help='Prepara y comprueba un perfil ficticio; las acciones se realizan por Computer Use.')
    parser.add_argument('--quote-scenario', action='store_true',
                        help='Cita entre dos sesiones ficticias, sin activar otra sesión o consultar modelos.')
    args = parser.parse_args()
    if args.quote_scenario and args.profile_scenario:
        parser.error('Elige un escenario de cita o perfil por instancia.')
    source_bundle = ROOT / 'frontend/build/macos/Build/Products/Release/Gennie.app'
    source_app = source_bundle / 'Contents/MacOS/Gennie'
    if not source_app.is_file():
        raise SystemExit('Falta el paquete release; usa scripts/build.sh antes de la sonda.')
    output = ROOT / '.verification/native-ui' / str(uuid.uuid4())
    output.mkdir(parents=True, mode=0o700)
    manifest = {'source_app':str(source_app),'source_app_sha256':hashlib.sha256(source_app.read_bytes()).hexdigest(),
                'model_modules_enabled':False,'personal_database_opened':False,
                'artifact_directory':str(output),
                'duration_limit_seconds':180 if args.profile_scenario or args.quote_scenario else 60}
    with tempfile.TemporaryDirectory(prefix='gennie-native-qa-') as directory:
        directory = Path(directory)
        # Identidad exclusiva: nunca enfocar/cerrar la instancia Debug del usuario
        # cuando la herramienta resuelve activación por bundle en vez de por ruta.
        bundle = directory / 'GennieQA.app'
        shutil.copytree(source_bundle, bundle, symlinks=True)
        bundle_id = 'dev.fer.gennie.qa.' + uuid.uuid4().hex
        info_path = bundle / 'Contents/Info.plist'
        with info_path.open('rb') as file:
            info = plistlib.load(file)
        info['CFBundleIdentifier'] = bundle_id
        info['CFBundleName'] = 'Gennie QA'
        info['CFBundleDisplayName'] = 'Gennie QA ' + bundle_id.rsplit('.', 1)[-1][:6]
        with info_path.open('wb') as file:
            plistlib.dump(info, file, fmt=plistlib.FMT_BINARY, sort_keys=False)
        subprocess.run(['codesign', '--force', '--sign', '-', '--entitlements',
                        str(ROOT / 'frontend/macos/Runner/Release.entitlements'), str(bundle)],
                       check=True, timeout=15, capture_output=True)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)],
                       check=True, timeout=15, capture_output=True)
        app = bundle / 'Contents/MacOS/Gennie'
        manifest.update(app=str(app), app_sha256=hashlib.sha256(app.read_bytes()).hexdigest(),
                        bundle_id=bundle_id, bundle_metadata_isolated=True)
        database, _ = fixture(directory)
        with sqlite3.connect(database) as connection:
            connection.execute("UPDATE presence_conversations SET provider='codex',provider_thread_id='native-fixture' WHERE id=1")
            connection.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(1,'user','Revisa los encargos sin cambiar permisos ni activar nuevas acciones.')")
            connection.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(1,'assistant','He preparado borradores para que puedas revisarlos. Los planes conservan sus permisos y necesitan tu confirmación antes de aplicar los cambios.')")
            if args.quote_scenario:
                connection.execute("INSERT INTO presence_conversations(id,title,provider,provider_thread_id) VALUES(2,'Sesión actual de prueba','codex','native-fixture-active')")
                connection.execute("UPDATE presence_history SET conversation_id=2 WHERE speaker='assistant'")
                connection.execute('UPDATE presence_current_conversation SET conversation_id=2 WHERE id=1')
                source, speaker, text, created = connection.execute("SELECT id,speaker,text,created_at FROM presence_history WHERE speaker='user'").fetchone()
                target = connection.execute("SELECT id FROM presence_history WHERE speaker='assistant'").fetchone()[0]
                connection.execute('INSERT INTO presence_message_replies(message_id,source_message_id,speaker,text,quoted_created_at,truncated) VALUES(?,?,?,?,?,0)',
                                   (target,source,speaker,text,created))
                manifest.update(chat_scenario='quote', quote_before=quote_record(connection))
            if args.profile_scenario:
                image = directory / 'avatar-qa.png'
                shutil.copyfile(ROOT / 'frontend/macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_128.png', image)
                manifest.update(profile_scenario=args.profile_scenario, fixture_image=str(image))
                if args.profile_scenario == 'restore':
                    connection.execute('UPDATE presence_assistants SET name=?,avatar_png=?,avatar_revision=1 WHERE id=?',
                                       ('Prueba local', image.read_bytes(), 'gennie'))
                manifest['profile_before'] = profile_record(connection)
        env = os.environ.copy()
        env['GENNIE_DATABASE_PATH'] = str(database)
        env.pop('GENNIE_OPENROUTER_API_KEY', None)
        with (output / 'app.log').open('x') as log:
            child = subprocess.Popen([str(app)], cwd=directory, env=env, stdout=log, stderr=subprocess.STDOUT)
            manifest['pid'] = child.pid
            print(json.dumps(manifest, ensure_ascii=False), flush=True)
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(sys.stdin, selectors.EVENT_READ)
                    if selector.select(timeout=manifest['duration_limit_seconds']):
                        sys.stdin.readline()
            except OSError:
                # stdin puede ser /dev/null en ejecución sin PTY. Conservar el
                # fallo y pasar igualmente por recogida, limpieza y manifiesto.
                manifest['input_wait_error'] = 'stdin_unavailable'
            finally:
                stop = 'already_exited'
                if child.poll() is None:
                    # La prueba pide Salir mediante Computer Use antes del límite.
                    # Este fallback recoge solo el hijo propio y no acredita cierre por UI.
                    stop = 'terminated_without_ui_quit'
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        stop = 'killed_after_termination_timeout'
                        child.kill()
                        child.wait(timeout=5)
                manifest.update(exit_code=child.returncode, stop=stop)
                manifest['qa_residual_processes'] = stop_qa_residuals(app)
                with sqlite3.connect(database) as connection:
                    tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
                    manifest.update(integrity=connection.execute('PRAGMA integrity_check').fetchone()[0],
                                    foreign_keys=connection.execute('PRAGMA foreign_key_check').fetchall(),
                                    messages=connection.execute('SELECT count(*) FROM presence_history').fetchone()[0],
                                    assignment_revisions=connection.execute('SELECT min(revision),max(revision) FROM assignments').fetchone(),
                                    jobs=connection.execute('SELECT count(*) FROM plugin_build_jobs').fetchone()[0] if 'plugin_build_jobs' in tables else 0,
                                    runs=connection.execute('SELECT count(*) FROM assignment_execution_runs').fetchone()[0])
                    if args.profile_scenario:
                        manifest['profile_after'] = profile_record(connection)
                    if args.quote_scenario:
                        manifest['quote_after'] = quote_record(connection)
    manifest['fixture_removed'] = not directory.exists()
    # Solo nombres UUID creados por este ensayo; no preferencias del bundle real.
    preferences = Path.home() / 'Library/Preferences' / (bundle_id + '.plist')
    preferences.unlink(missing_ok=True)
    saved_state = Path.home() / 'Library/Saved Application State' / (bundle_id + '.savedState')
    if saved_state.exists():
        shutil.rmtree(saved_state)
    manifest['qa_preferences_removed'] = not preferences.exists() and not saved_state.exists()
    log_text = (output / 'app.log').read_text()
    manifest['engine_channel_errors'] = engine_errors(log_text)
    if args.profile_scenario:
        profile = manifest['profile_after']
        expected_name = 'Gennie' if args.profile_scenario == 'cancel' else 'Prueba local'
        expected_revision = {'save': 1, 'cancel': 0, 'restore': 2}[args.profile_scenario]
        manifest['profile_check_passed'] = (
            profile['id'] == 'gennie' and profile['name'] == expected_name
            and profile['avatar_revision'] == expected_revision
            and (profile['avatar_bytes'] > 0) == (args.profile_scenario == 'save')
            and (args.profile_scenario != 'cancel' or profile == manifest['profile_before'])
            and manifest['messages'] == 2 and manifest['assignment_revisions'] == (1, 1)
            and manifest['jobs'] == 0 and manifest['runs'] == 0
            and manifest['fixture_removed'] and manifest['qa_preferences_removed']
        )
    if args.quote_scenario:
        manifest['quote_check_passed'] = (
            manifest['quote_before'] == manifest['quote_after']
            and manifest['quote_after']['active_conversation'] == 2
            and manifest['messages'] == 2 and manifest['assignment_revisions'] == (1,1)
            and manifest['jobs'] == 0 and manifest['runs'] == 0
            and manifest['fixture_removed'] and manifest['qa_preferences_removed']
        )
    with (output / 'manifest.json').open('x') as file:
        json.dump(manifest, file, ensure_ascii=False, indent=2)
        file.write('\n')
    print(json.dumps(manifest, ensure_ascii=False), flush=True)
    if manifest.get('input_wait_error'):
        raise SystemExit('Fallo: stdin no disponible; ejecuta el ensayo con PTY.')
    if args.require_clean_quit:
        require_clean_quit(manifest, log_text)
    if args.profile_scenario and not manifest['profile_check_passed']:
        raise SystemExit('Fallo: el perfil guardado no coincide con el escenario nativo.')
    if args.quote_scenario and not manifest['quote_check_passed']:
        raise SystemExit('Fallo: la cita o sus sesiones cambiaron durante la navegación.')


if __name__ == '__main__':
    main()

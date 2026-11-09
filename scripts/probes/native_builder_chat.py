#!/usr/bin/env python3
"""Copia nativa aislada para probar el chat y el constructor con CUA."""
import json, os, plistlib, shutil, sqlite3, subprocess, sys, tempfile, threading, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from bot_notice_bridge import ROOT, migrate

class Site(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/public':
            body = '<html><body><h1>Catálogo público</h1><p>AZUL-42</p></body></html>'
        else:
            body = '<html><body><h1>Acceso privado</h1><p>Esta sección requiere iniciar sesión.</p><input placeholder="Usuario"><input placeholder="Contraseña" type="password"></body></html>'
        data = body.encode()
        self.send_response(200); self.send_header('Content-Type', 'text/html; charset=utf-8'); self.send_header('Cache-Control', 'no-store'); self.send_header('Content-Length', str(len(data))); self.end_headers(); self.wfile.write(data)
    def log_message(self, *_): pass

source = ROOT / 'frontend/build/macos/Build/Products/Release/Gennie.app'
original_home = Path.home()
with tempfile.TemporaryDirectory(prefix='gennie-native-builder-') as tmp:
    root = Path(tmp); home = root / 'home'; home.mkdir(mode=0o700)
    codex = root / 'codex'; codex.mkdir(mode=0o700)
    shutil.copyfile(original_home / '.codex/auth.json', codex / 'auth.json'); (codex / 'auth.json').chmod(0o600)
    browser = home / 'Library/Application Support/Gennie/browser'; browser.mkdir(parents=True)
    for name in ('runtime-patchright-1.64.2', 'browsers'):
        (browser / name).symlink_to(original_home / 'Library/Application Support/Gennie/browser' / name, target_is_directory=True)
    bundle = root / 'GennieQA.app'; shutil.copytree(source, bundle, symlinks=True)
    info_path = bundle / 'Contents/Info.plist'
    with info_path.open('rb') as f: info = plistlib.load(f)
    bundle_id = 'dev.fer.gennie.qa.' + uuid.uuid4().hex
    info['CFBundleIdentifier'] = bundle_id; info['CFBundleName'] = 'Gennie QA'; info['CFBundleDisplayName'] = 'Gennie QA'
    with info_path.open('wb') as f: plistlib.dump(info, f, fmt=plistlib.FMT_BINARY, sort_keys=False)
    subprocess.run(['codesign', '--force', '--sign', '-', '--entitlements', str(ROOT / 'frontend/macos/Runner/Release.entitlements'), str(bundle)], capture_output=True, check=True, timeout=20)
    subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], capture_output=True, check=True, timeout=20)
    database = root / 'fixture.sqlite3'
    with sqlite3.connect(database) as c:
        migrate(c, 'bootstrap', ROOT / 'bootstrap/migrations')
        for name in ('transcription','decisions','llm','tts','reminders_plugin','reminders','presence','assignments','browser','plugins','proactivity'):
            c.execute('INSERT INTO modules(name,active) VALUES(?,?)', (name, name in ('llm','presence','browser','plugins')))
        c.commit()
    server = ThreadingHTTPServer(('127.0.0.1', 0), Site)
    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    env = os.environ.copy(); env.update(HOME=str(home), CODEX_HOME=str(codex), GENNIE_DATABASE_PATH=str(database))
    for key in ('OPENAI_API_KEY', 'OPENROUTER_API_KEY', 'GENNIE_OPENROUTER_API_KEY'): env.pop(key, None)
    log = (root / 'app.log').open('w')
    app = subprocess.Popen([str(bundle / 'Contents/MacOS/Gennie')], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    print(json.dumps({'bundle_id':bundle_id,'bundle':str(bundle),'pid':app.pid,'database':str(database),'origin':f'http://127.0.0.1:{server.server_port}','log':str(root/'app.log')}), flush=True)
    try:
        sys.stdin.readline()
    finally:
        if app.poll() is None:
            app.terminate()
            try: app.wait(timeout=40)
            except subprocess.TimeoutExpired: app.kill(); app.wait(timeout=5)
        server.shutdown(); server.server_close(); thread.join(timeout=3); log.close()
        with sqlite3.connect(database) as c:
            print(json.dumps({'exit_code':app.returncode,'jobs':c.execute('SELECT state,summary FROM plugin_build_jobs').fetchall(),'messages':c.execute('SELECT state,message FROM plugin_builder_messages ORDER BY id').fetchall()}, ensure_ascii=False),flush=True)

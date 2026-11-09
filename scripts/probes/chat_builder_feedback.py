#!/usr/bin/env python3
"""Chat y constructor reales con base, perfil y servidor de prueba temporales."""
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
original_home = Path.home()
with tempfile.TemporaryDirectory(prefix='gennie-chat-builder-') as temp:
    temporary = Path(temp)
    home = temporary / 'home'
    codex = temporary / 'codex'
    home.mkdir(mode=0o700)
    codex.mkdir(mode=0o700)
    shutil.copyfile(original_home / '.codex/auth.json', codex / 'auth.json')
    (codex / 'auth.json').chmod(0o600)
    browser = home / 'Library/Application Support/Gennie/browser'
    browser.mkdir(parents=True)
    for name in ('runtime-patchright-1.64.2', 'browsers'):
        (browser / name).symlink_to(original_home / 'Library/Application Support/Gennie/browser' / name, target_is_directory=True)
    env = os.environ.copy()
    env.update(HOME=str(home), CODEX_HOME=str(codex), CARGO_HOME=str(original_home / '.cargo'),
               RUSTUP_HOME=str(original_home / '.rustup'), GENNIE_TEST_HELPER=str(root / 'target/flutter/debug/gennie'))
    for name in ('OPENAI_API_KEY', 'OPENROUTER_API_KEY', 'GENNIE_OPENROUTER_API_KEY'):
        env.pop(name, None)
    command = ['cargo', 'test', '-p', 'gennie', '--locked', 'real_chat_builder_feedback_probe', '--', '--ignored', '--nocapture']
    process = subprocess.Popen(command, cwd=root, env=env, start_new_session=True)
    try:
        code = process.wait(timeout=420)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=10)
        raise SystemExit('La sonda agotó su límite de observación')
    raise SystemExit(code)

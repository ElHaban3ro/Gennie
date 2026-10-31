#!/usr/bin/env python3
"""Comprueba configuración de sesiones efímeras sin cuenta ni inferencia."""
import json
import os
from pathlib import Path
import selectors
import subprocess
import tempfile
import time

BINARY = Path('/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex')
CONFIG = {
    'features.shell_tool': False, 'features.unified_exec': False,
    'features.apply_patch_freeform': False, 'features.multi_agent': False,
    'features.computer_use': False, 'features.browser_use': False,
    'features.browser_use_external': False, 'features.apps': False,
    'features.plugins': False, 'web_search': 'disabled',
}

def run():
    with tempfile.TemporaryDirectory(prefix='gennie-mediation-protocol-') as directory:
        child_env = {**os.environ, 'CODEX_HOME': directory}
        for name in ('OPENAI_API_KEY', 'CODEX_API_KEY', 'AZURE_OPENAI_API_KEY'):
            child_env.pop(name, None)
        process = subprocess.Popen([
            str(BINARY), 'app-server', '--listen', 'stdio://',
            '-c', 'permissions.gennie_title.filesystem={":root"="deny",":minimal"="read"}',
            '-c', 'permissions.gennie_title.network.enabled=false',
            '-c', 'default_permissions="gennie_title"',
            '-c', 'features.shell_tool=false', '-c', 'features.unified_exec=false',
            '-c', 'features.plugins=false', '-c', 'features.apps=false',
            '-c', 'web_search="disabled"',
        ], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            env=child_env, cwd=directory, bufsize=0)
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        buffer = b''
        def request(identifier, method, params):
            nonlocal buffer
            process.stdin.write((json.dumps({'id':identifier, 'method':method, 'params':params})+'\n').encode())
            process.stdin.flush()
            deadline = time.monotonic()+10
            while time.monotonic() < deadline:
                if b'\n' in buffer:
                    line, buffer = buffer.split(b'\n',1)
                    event = json.loads(line)
                    assert event.get('method') != 'turn/started', 'Turno inesperado'
                    if event.get('id') == identifier:
                        return event
                    continue
                if selector.select(max(0,deadline-time.monotonic())):
                    data = os.read(process.stdout.fileno(),65536)
                    if not data:
                        raise RuntimeError('El proceso terminó antes de responder')
                    buffer += data
            raise TimeoutError('Sin respuesta del protocolo')
        try:
            assert 'result' in request(1,'initialize',{'clientInfo':{'name':'gennie-mediation-probe','version':'1'},'capabilities':{'experimentalApi':True}})
            process.stdin.write(b'{"method":"initialized","params":{}}\n');process.stdin.flush()
            account = request(2,'account/read',{'refreshToken':False})
            assert account['result']['account'] is None, 'La sonda exige un perfil sin cuenta'
            for index, phantom in enumerate((True,False),start=3):
                config = dict(CONFIG)
                if phantom:
                    config['mcp_servers.gennie.enabled'] = False
                response = request(index,'thread/start',{
                    'cwd':directory, 'model':'gpt-6-luna', 'ephemeral':True,
                    'approvalPolicy':'never','permissions':'gennie_title',
                    'baseInstructions':'Devuelve únicamente un mensaje JSON de prueba; no uses herramientas.',
                    'developerInstructions':'Redacción de un mensaje, sin acciones externas.',
                    'config':config,
                })
                print(json.dumps({'phantom_mcp':phantom,'accepted':'result' in response,'error':response.get('error')},ensure_ascii=False),flush=True)
                assert ('result' in response) == (not phantom), 'La configuración no coincide con el contrato observado'
                if 'result' in response:
                    assert response['result']['thread']['ephemeral'] is True
        finally:
            selector.close();process.stdin.close();process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill();process.wait()

if __name__ == '__main__':
    run()

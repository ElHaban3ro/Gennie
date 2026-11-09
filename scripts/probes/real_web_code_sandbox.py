#!/usr/bin/env python3
"""Ejecuta JavaScript de prueba con permisos mínimos de Deno y un dominio permitido."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import sys

MAX_CODE_BYTES=32_000
MAX_OUTPUT_BYTES=1_000_000


def execute(source,proxy_port,timeout=25):
    if not isinstance(source,str) or not source.strip() or len(source.encode())>MAX_CODE_BYTES:
        raise ValueError('invalid_code')
    if type(proxy_port) is not int or not 1 <= proxy_port <= 65535:
        raise ValueError('invalid_proxy')
    executable=shutil.which('deno')
    if not executable:
        raise RuntimeError('deno_unavailable')
    with tempfile.TemporaryDirectory(prefix='gennie-real-code-',dir='/tmp') as tmp:
        root=Path(tmp)
        bootstrap='''(() => {
  const originalFetch = globalThis.fetch;
  Object.defineProperty(globalThis, 'fetch', {configurable:false,writable:false,value: async (input, options={}) => {
    if (typeof input !== 'string' && !(input instanceof URL)) throw new Error('Solo se aceptan URLs explícitas');
    const u=new URL(input.toString());
    if (u.origin!=='https://quotes.toscrape.com' || u.search || u.hash || (options.method && options.method.toUpperCase()!=='GET') || options.body) throw new Error('Destino o método no permitido');
    const route=(u.pathname==='/' || u.pathname==='/page/1/') ? '/page/1' : u.pathname==='/page/2/' ? '/page/2' : null;
    if (!route) throw new Error('Página no permitida');
    return originalFetch('http://127.0.0.1:PORT'+route);
  }});
})();
'''.replace('PORT',str(proxy_port))
        script=root/'solution.js';script.write_text(bootstrap+'\n'+source,encoding='utf-8')
        stdout=root/'stdout';stderr=root/'stderr'
        env={'HOME':str(root),'DENO_DIR':str(root/'deno-cache'),'PATH':'/usr/bin:/bin','LANG':'C.UTF-8'}
        command=[executable,'run','--no-config','--no-remote','--no-npm','--no-prompt',
                 '--allow-net=127.0.0.1:'+str(proxy_port),'--deny-read','--deny-write','--deny-env','--deny-run','--deny-ffi',str(script)]
        started=time.monotonic()
        with stdout.open('wb') as out,stderr.open('wb') as err:
            launcher=Path(__file__).with_name('real_web_code_launcher.py')
            process=subprocess.Popen([sys.executable,str(launcher),executable,str(timeout),*command[1:]],
                                     cwd=root,env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,start_new_session=True)
            try: code=process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL);process.wait();code=None
        data=stdout.read_bytes();error=stderr.read_bytes()
        try: value=json.loads(data)
        except (ValueError,UnicodeError): value=None
        return {'exit_code':code,'elapsed_ms':round((time.monotonic()-started)*1000),
                'output':value,'stdout_bytes':len(data),'stderr_bytes':len(error),
                'stderr_excerpt':error.decode(errors='replace')[:600] if code!=0 else None}

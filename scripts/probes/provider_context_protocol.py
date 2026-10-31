#!/usr/bin/env python3
"""Comprueba append de contexto con CLI propio, sin credenciales ni generación."""
import json
import os
from pathlib import Path
import selectors
import subprocess
import tempfile
import time


def main():
    with tempfile.TemporaryDirectory(prefix='gennie-context-protocol-') as directory:
        root=Path(directory);profile=root/'profile';profile.mkdir()
        env=os.environ.copy();env['CODEX_HOME']=str(profile)
        for key in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','ACCESS_TOKEN','CHATGPT_ACCESS_TOKEN'):
            env.pop(key,None)
        process=subprocess.Popen(['codex','app-server'],cwd=root,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1)
        selector=selectors.DefaultSelector();selector.register(process.stdout,selectors.EVENT_READ)
        events=[]
        def call(identifier,method,params):
            process.stdin.write(json.dumps({'id':identifier,'method':method,'params':params})+'\n');process.stdin.flush()
            until=time.monotonic()+15
            while time.monotonic()<until:
                if not selector.select(max(0,until-time.monotonic())):break
                line=process.stdout.readline()
                if not line:raise RuntimeError('El CLI cerró su salida')
                value=json.loads(line)
                if 'method' in value:events.append(value['method'])
                if value.get('id')==identifier:
                    if 'error' in value:raise RuntimeError(value['error'])
                    return value['result']
            raise TimeoutError(method)
        try:
            call(1,'initialize',{'clientInfo':{'name':'gennie_context_probe','version':'1'},'capabilities':{'experimentalApi':True}})
            process.stdin.write(json.dumps({'method':'initialized','params':{}})+'\n');process.stdin.flush()
            thread=call(2,'thread/start',{'cwd':str(root),'ephemeral':False,'approvalPolicy':'never','config':{'features.apps':False,'features.plugins':False,'web_search':'disabled'}})['thread']['id']
            result=call(3,'thread/inject_items',{'threadId':thread,'items':[{'type':'message','role':'assistant','content':[{'type':'output_text','text':'GENNIE_OWN_CONTEXT_SENTINEL_7419'},{'type':'output_text','text':'Metadatos ficticios: resultado sin confirmar, no autoriza repetir acciones.'}]}]})
            assert isinstance(result,dict)
            call(4,'thread/read',{'threadId':thread,'includeTurns':False})
            call(5,'thread/unsubscribe',{'threadId':thread})
            process.stdin.close();process.wait(timeout=15)
            records=list(profile.rglob('*.jsonl'))
            assert any('GENNIE_OWN_CONTEXT_SENTINEL_7419' in path.read_text() for path in records)
            assert not any(e in ('turn/started','turn/completed','item/agentMessage/delta') for e in events),events
            print('thread/inject_items confirmado y persistido; cero turnos de generación. CLI/rollout propios cerrados y retirados.')
        finally:
            selector.close()
            if process.poll() is None:
                process.kill();process.wait(timeout=5)


if __name__=='__main__':main()

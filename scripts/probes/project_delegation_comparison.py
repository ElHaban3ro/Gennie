#!/usr/bin/env python3
"""Compara una auditoría de código real con delegación y límites explícitos."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

import assignment_evaluation_worker as worker
import pilot_gennie_real

ROOT = Path(__file__).resolve().parents[2]
FILES = ['apps/llm/src/codex.rs', 'apps/llm/src/turn.rs',
         'apps/llm/src/provider_probe.rs', 'apps/llm/src/provider_real_web_probe.rs',
         'apps/llm/src/provider_turn_observation.rs', 'apps/llm/src/tool_use.rs',
         'kernel/mcp/src/lib.rs', 'kernel/mcp/src/connection.rs',
         'kernel/mcp/src/stdio_bridge.rs', 'kernel/mcp/src/cancellation.rs']
OBJECTIVE = ("Audita el código real disponible en este directorio, exclusivamente en modo lectura. "
 "La meta es investigar por qué una tarea de agente podría detenerse antes de terminar. "
 "Delega a dos subagentes reales: uno debe examinar temporizadores y cancelación del turno, "
 "y otro el transporte MCP y registro de herramientas; dales tareas concretas y espera sus resultados. "
 "Después contrasta personalmente sus hallazgos con el código y entrega un informe integrado en español. "
 "Incluye: recorrido del turno desde la petición hasta su cierre; límites de tiempo y quién los impone; "
 "qué señales prueban o no que se cumplió el objetivo; tres riesgos concretos con archivo y número de línea; "
 "dos hipótesis alternativas que no se pueden descartar; y una propuesta de pruebas reproducibles sin datos personales. "
 "Separa defectos probados de posibilidades y no inventes resultados de ejecución. "
 "No cambies archivos ni consultes datos ajenos a este directorio.")


class Client(worker.Client):
    def __init__(self, process, until):
        super().__init__(process, until)
        self.usage_by_thread = {}
        self.collaboration = []
        self.command_count = 0

    def observe(self, value):
        method = value.get('method')
        params = value.get('params', {})
        thread = params.get('threadId')
        if method == 'thread/tokenUsage/updated' and isinstance(thread, str):
            self.usage_by_thread[thread] = worker.usage_snapshot(params.get('tokenUsage'))
        if method == 'turn/completed':
            turn = params.get('turn', {})
            if isinstance(turn.get('id'), str): self.completed_turns[turn['id']] = turn.get('status')
        item = params.get('item', {})
        if method in ('item/started', 'item/completed') and isinstance(item, dict):
            kind = item.get('type')
            if kind in ('collabAgentToolCall', 'subAgentActivity'):
                self.collaboration.append({'method': method, 'thread': thread,
                    'tool': item.get('tool'), 'status': item.get('status'),
                    'receivers': item.get('receiverThreadIds'), 'agent': item.get('agentThreadId')})
            if kind == 'commandExecution' and method == 'item/completed': self.command_count += 1
            if thread == self.thread_id and kind == 'agentMessage' and method == 'item/completed':
                text = item.get('text')
                if isinstance(text, str): self.messages[item.get('id', str(len(self.messages)))] = text


def run_direct(base, catalog, output):
    home = base/'cli-home'; home.mkdir(mode=0o700)
    auth = Path(os.environ.get('CODEX_HOME', str(Path.home()/'.codex')))/'auth.json'
    (home/'auth.json').symlink_to(auth)
    env = os.environ.copy(); env['CODEX_HOME'] = str(home)
    for key in ('OPENAI_API_KEY', 'OPENROUTER_API_KEY', 'GENNIE_OPENROUTER_API_KEY'):
        env.pop(key, None)
    configs = ['features.multi_agent=true', 'agents.max_depth=1', 'agents.max_threads=3',
               'model_catalog_json='+json.dumps(str(catalog)), 'web_search="disabled"',
               'features.plugins=false', 'features.apps=false', 'model_reasoning_effort="medium"']
    command = [worker.binary(), 'app-server', '--listen', 'stdio://']
    for config in configs: command.extend(['-c', config])
    began = time.monotonic()
    process = subprocess.Popen(command,cwd=base/'workspace',env=env,stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,start_new_session=True)
    client = Client(process, began+180)
    report = {'status':'not_started'}
    try:
        client.call(1,'initialize',{'clientInfo':{'name':'project_delegation_comparison','version':'1'},'capabilities':{'experimentalApi':True}})
        client.send({'method':'initialized','params':{}})
        thread = client.call(2,'thread/start',{'cwd':str(base/'workspace'),'model':'gpt-6.1-terra',
            'approvalPolicy':'never','sandbox':'read-only'})['thread']['id']
        client.thread_id = thread
        client.generation_requested = True
        turn = client.call(3,'turn/start',{'threadId':thread,'cwd':str(base/'workspace'),
            'model':'gpt-6.1-terra','effort':'medium','approvalPolicy':'never',
            'input':[{'type':'text','text':OBJECTIVE}]})['turn']['id']
        while turn not in client.completed_turns: client.next()
        report.update(status='turn_finished', turn_status=client.completed_turns[turn], thread_id=thread)
    except worker.Failure as error:
        report.update(status='runtime_failure', error=str(error))
    finally:
        report.update(usage_by_thread=client.usage_by_thread,collaboration=client.collaboration,
                      command_count=client.command_count,messages=list(client.messages.values()))
        client.selector.close()
        if process.poll() is None:
            os.killpg(process.pid,signal.SIGTERM)
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: os.killpg(process.pid,signal.SIGKILL);process.wait(timeout=5)
    output.write_text(json.dumps(report,ensure_ascii=False));output.chmod(0o600)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--arm',choices=('gennie','direct'),required=True);args=parser.parse_args()
    directory=ROOT/'.verification/project-delegation'/str(uuid.uuid4());directory.mkdir(parents=True,mode=0o700)
    with tempfile.TemporaryDirectory(prefix='gennie-project-',dir='/tmp') as temp:
        base=Path(temp);workspace=base/'workspace';workspace.mkdir(mode=0o700)
        hashes={}
        for name in FILES:
            source=ROOT/name; target=workspace/name;target.parent.mkdir(parents=True,exist_ok=True)
            if name == 'apps/llm/src/provider_turn_observation.rs' and os.environ.get('GENNIE_PROJECT_OBSERVATION_SOURCE'):
                source = Path(os.environ['GENNIE_PROJECT_OBSERVATION_SOURCE'])
            data=source.read_bytes();target.write_bytes(data);hashes[name]=hashlib.sha256(data).hexdigest()
        catalog=base/'models.json';shutil.copyfile(Path.home()/'.codex/models_cache.json',catalog)
        output=base/'result.json'
        profile=pilot_gennie_real.settings(ROOT/'data/soc.sqlite3','gennie')
        if profile['model']!='gpt-6.1-terra' or profile['effort']!='medium': raise RuntimeError('model_mismatch')
        started=time.monotonic()
        if args.arm=='gennie':
            settings=base/'settings.json';settings.write_text(json.dumps(profile,ensure_ascii=False));settings.chmod(0o600)
            prompt=base/'prompt.txt';prompt.write_text(OBJECTIVE)
            env=os.environ.copy()
            for key in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY'):env.pop(key,None)
            env.update(GENNIE_PROJECT_BASE=str(base),GENNIE_PROJECT_SETTINGS=str(settings),
                GENNIE_PROJECT_CATALOG=str(catalog),GENNIE_PROJECT_OUTPUT=str(output),GENNIE_PROJECT_PROMPT=str(prompt))
            process=subprocess.Popen(['cargo','test','-p','gennie-llm','--locked','project_real_gennie_subagents',
                '--','--ignored','--nocapture'],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
            limit = 660 if os.environ.get('GENNIE_PROJECT_LONG') == '1' else 210
            try:stdout,stderr=process.communicate(timeout=limit);exit_code=process.returncode
            except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);stdout,stderr=process.communicate();exit_code=None
        else:
            run_direct(base,catalog,output);exit_code=0
        elapsed=round((time.monotonic()-started)*1000)
        result=json.loads(output.read_text()) if output.exists() else {'status':'output_missing'}
        after={name:hashlib.sha256((workspace/name).read_bytes()).hexdigest() for name in FILES}
        report={'arm':args.arm,'elapsed_ms':elapsed,'exit_code':exit_code,'result':result,
            'snapshot':hashes,'snapshot_unchanged':hashes==after,'model':profile['model'],'effort':profile['effort'],
            'profile_characters':len(profile['instructions'] or '')+len(profile['personality'] or ''),
            'prompt_sha256':hashlib.sha256(OBJECTIVE.encode()).hexdigest()}
    path=directory/'result.json';path.write_text(json.dumps(report,ensure_ascii=False));path.chmod(0o600)
    print(json.dumps({'directory':str(directory),'status':result.get('status'),'elapsed_ms':elapsed,'exit_code':exit_code}))


if __name__=='__main__': main()

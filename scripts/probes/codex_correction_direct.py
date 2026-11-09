#!/usr/bin/env python3
"""Dos turnos de Codex directo sobre una corrección de objetivo ficticia."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import uuid

import assignment_evaluation as common
import continuity_baseline
import codex_comparison_direct as direct

PROMPTS = ['Consulta con probe_read la fuente ficticia alpha y dime su valor exacto.',
           'Cambio de objetivo: ahora necesito únicamente el valor de beta. Consulta beta con probe_read; no repitas la consulta de alpha.']


def main():
    output = direct.ROOT/'.verification/codex-correction-direct'/str(uuid.uuid4())
    output.mkdir(parents=True,mode=0o700)
    plan = {'model':'gpt-6-luna','effort':'low','turns_max':2,'timeout_seconds':60,'prompts':PROMPTS,
            'server_sha256':common.sha_file(direct.SERVER),'codex_sha256':common.sha_file(Path(common.codex_binary())),
            'scope':'MCP ficticio de lectura; hilo temporal; sin efectos externos'}
    common.save(output/'plan.json',plan)
    auth = Path(os.environ.get('CODEX_HOME',str(Path.home()/'.codex')))/'auth.json'
    if not auth.is_file():
        common.save(output/'result.json',{'status':'not_run','reason':'authentication_not_available','turns_started':0})
        print(json.dumps({'directory':str(output),'status':'not_run'}))
        return 1
    began = time.monotonic()
    turns = []
    with tempfile.TemporaryDirectory(prefix='gennie-codex-correction-') as temporary:
        root = Path(temporary)
        home, work = root/'home', root/'work'
        home.mkdir(mode=0o700)
        work.mkdir(mode=0o700)
        (home/'auth.json').symlink_to(auth)
        trace = root/'trace.jsonl'
        env = os.environ.copy()
        env['CODEX_HOME'] = str(home)
        for name in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_DATABASE_PATH'):
            env.pop(name,None)
        config = [f'mcp_servers.gennie.command={json.dumps(sys.executable)}',
                  f'mcp_servers.gennie.args={json.dumps([str(direct.SERVER)])}',
                  f'mcp_servers.gennie.env.CONTINUITY_COMPARISON_TRACE={json.dumps(str(trace))}',
                  'features.shell_tool=false','features.unified_exec=false','features.computer_use=false',
                  'features.browser_use=false','features.browser_use_external=false','features.multi_agent=false',
                  'features.plugins=false','features.apps=false','web_search="disabled"','model_reasoning_effort="low"']
        thread_id = None
        for index,prompt in enumerate(PROMPTS):
            last = root/f'last-{index}.txt'
            command = [common.codex_binary(),'exec','--ignore-user-config','--skip-git-repo-check',
                       '--json','-s','read-only','-m',plan['model'],'-C',str(work),'-o',str(last)]
            for item in config:
                command.extend(['-c',item])
            if index:
                command.extend(['resume',thread_id,prompt])
            else:
                command.append(prompt)
            if index:
                with trace.open('a',encoding='utf-8') as stream:
                    stream.write(json.dumps({'event':'correction'})+'\n')
            process = subprocess.Popen(command,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                                       text=True,start_new_session=True)
            try:
                events,_ = process.communicate(timeout=60)
                code = process.returncode
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL)
                events,_ = process.communicate()
                code = None
            try:
                os.killpg(process.pid,0)
            except ProcessLookupError:
                group_clean = True
            else:
                os.killpg(process.pid,signal.SIGKILL)
                group_clean = False
            parsed = [json.loads(line) for line in events.splitlines() if line.strip()]
            identifiers = [e.get('thread_id') for e in parsed if e.get('type') == 'thread.started']
            observed_id = identifiers[0] if len(identifiers)==1 and isinstance(identifiers[0],str) else None
            if index == 0:
                thread_id = observed_id
            same_thread = observed_id is not None and observed_id == thread_id
            response = last.read_text() if last.exists() else ''
            if response:
                with trace.open('a',encoding='utf-8') as stream:
                    stream.write(json.dumps({'event':'reply','text':response},ensure_ascii=False)+'\n')
            turns.append({'exit_code':code,'group_clean':group_clean,'same_thread':same_thread,
                          'response_present':bool(response.strip())})
            if code != 0 or not same_thread or not response.strip() or not group_clean:
                break
        rows = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
        report = {'status':'completed' if len(turns)==2 and all(t['exit_code']==0 and t['group_clean'] and t['same_thread'] and t['response_present'] for t in turns) else 'inconclusive',
                  'turns_started':len(turns),'turns':turns,'trace':rows,
                  'evaluation':continuity_baseline.evaluate('correction',rows),
                  'elapsed_ms':round((time.monotonic()-began)*1000),'tokens':None,'cost':None,
                  'server_unchanged':common.sha_file(direct.SERVER)==plan['server_sha256'],
                  'codex_unchanged':common.sha_file(Path(common.codex_binary()))==plan['codex_sha256']}
    report['profile_removed'] = not root.exists()
    common.save(output/'result.json',report)
    print(json.dumps({'directory':str(output),'status':report['status'],'turns_started':len(turns),
                      'profile_removed':report['profile_removed']}))
    return 0 if report['status']=='completed' and report['profile_removed'] and report['server_unchanged'] and report['codex_unchanged'] else 1


if __name__ == '__main__':
    sys.exit(main())

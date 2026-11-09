#!/usr/bin/env python3
"""Comparación acotada de Codex exec con MCP ficticio, sin usar el checkout como workspace."""
import hashlib
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

ROOT = Path(__file__).resolve().parents[2]
SERVER = ROOT/'scripts/probes/codex_comparison_mcp.py'
PROMPT = 'Consulta las fuentes ficticias alpha y beta y dame sus dos valores exactos.'


def main():
    run_id = str(uuid.uuid4())
    output = ROOT/'.verification/codex-direct-comparison'/run_id
    output.mkdir(parents=True,mode=0o700)
    plan = {'model':'gpt-6-luna','effort':'low','turns_max':2,'timeout_seconds':60,
            'prompt':PROMPT,'server_sha256':common.sha_file(SERVER),
            'codex_sha256':common.sha_file(Path(common.codex_binary())),
            'scope':'MCP ficticio de lectura, sin Gennie ni efectos externos',
            'limitations':'Codex exec usa instrucciones y sandbox propios; no es equivalencia exacta con el host Gennie.'}
    common.save(output/'plan.json',plan)
    auth_source = Path(os.environ.get('CODEX_HOME',str(Path.home()/'.codex')))/'auth.json'
    if not auth_source.is_file():
        common.save(output/'result.json',{'status':'not_run','reason':'authentication_not_available','turns_started':0})
        print(json.dumps({'directory':str(output),'status':'not_run'}))
        return 1
    results = []
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='gennie-codex-direct-') as temporary:
        root = Path(temporary)
        for index in range(2):
            home = root/f'home-{index}'
            work = root/f'work-{index}'
            home.mkdir(mode=0o700)
            work.mkdir(mode=0o700)
            (home/'auth.json').symlink_to(auth_source)
            trace = root/f'trace-{index}.jsonl'
            last = root/f'last-{index}.txt'
            env = os.environ.copy()
            env['CODEX_HOME'] = str(home)
            for name in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_DATABASE_PATH'):
                env.pop(name,None)
            config = [
                f'mcp_servers.gennie.command={json.dumps(sys.executable)}',
                f'mcp_servers.gennie.args={json.dumps([str(SERVER)])}',
                f'mcp_servers.gennie.env.CONTINUITY_COMPARISON_TRACE={json.dumps(str(trace))}',
                'features.shell_tool=false','features.unified_exec=false',
                'features.computer_use=false','features.browser_use=false','features.browser_use_external=false',
                'features.multi_agent=false','features.plugins=false','features.apps=false',
                'web_search="disabled"','model_reasoning_effort="low"',
            ]
            command = [common.codex_binary(),'exec','--ignore-user-config','--ephemeral','--skip-git-repo-check',
                       '--json','-s','read-only','-m',plan['model'],'-C',str(work),'-o',str(last)]
            for item in config:
                command.extend(['-c',item])
            command.append(PROMPT)
            process = subprocess.Popen(command,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                                       text=True,start_new_session=True)
            try:
                events, _ = process.communicate(timeout=60)
                code = process.returncode
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL)
                events, _ = process.communicate()
                code = None
            try:
                os.killpg(process.pid,0)
            except ProcessLookupError:
                group_clean = True
            else:
                os.killpg(process.pid,signal.SIGKILL)
                group_clean = False
            rows = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
            response = last.read_text() if last.exists() else ''
            if response:
                rows.append({'event':'reply','text':response})
            discovered = any(event.get('event') == 'mcp_tools_list' for event in rows)
            results.append({'exit_code':code,'group_clean':group_clean,'tool_list_requested':discovered,'trace':rows,'evaluation':continuity_baseline.evaluate('two_sources',rows),
                            'events_sha256':hashlib.sha256(events.encode()).hexdigest(),
                            'response_present':bool(response.strip())})
            common.save(output/f'case-{index+1}.json',results[-1])
            if code != 0 or not response.strip() or not group_clean or not discovered:
                break
    report = {'status':'completed' if len(results)==2 and all(r['exit_code']==0 and r['response_present'] and r['group_clean'] and r['tool_list_requested'] for r in results) else 'inconclusive',
              'turns_started':len(results),'elapsed_ms':round((time.monotonic()-started)*1000),
              'cases':results,'profile_removed':not root.exists(),'tokens':None,'cost':None,
              'server_unchanged':common.sha_file(SERVER)==plan['server_sha256'],
              'codex_unchanged':common.sha_file(Path(common.codex_binary()))==plan['codex_sha256']}
    common.save(output/'result.json',report)
    print(json.dumps({'directory':str(output),'status':report['status'],'turns_started':len(results),
                      'profile_removed':report['profile_removed']}))
    return 0 if report['status']=='completed' and report['profile_removed'] and report['server_unchanged'] and report['codex_unchanged'] else 1


if __name__ == '__main__':
    sys.exit(main())

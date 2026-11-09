#!/usr/bin/env python3
"""Celda piloto con codex exec y el mismo fixture MCP de la arquitectura experimental."""
import argparse
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
import pilot_fixture
import pilot_mcp

ROOT = Path(__file__).resolve().parents[2]


def extract_usage(events):
    """Lee el último total de final de turno; conserva desconocidos."""
    values = [e.get('usage') for e in events if isinstance(e,dict) and e.get('type')=='turn.completed' and isinstance(e.get('usage'),dict)]
    if not values:
        return None
    v = values[-1]
    allowed = ('input_tokens','cached_input_tokens','output_tokens','reasoning_output_tokens','total_tokens')
    return {key:v[key] for key in allowed if type(v.get(key)) is int and v[key]>=0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case',required=True,choices=tuple(pilot_fixture.CASES))
    args = parser.parse_args()
    output = ROOT/'.verification/pilot-codex-direct'/str(uuid.uuid4())
    output.mkdir(parents=True,mode=0o700)
    server = Path(pilot_mcp.__file__).resolve()
    binary = Path(common.codex_binary())
    sources = [Path(__file__).resolve(),server,Path(pilot_fixture.__file__).resolve(),ROOT/'scripts/probes/pilot_web_fixture.py']
    fingerprints = {str(p.relative_to(ROOT)):common.sha_file(p) for p in sources}
    fingerprints['codex_binary'] = common.sha_file(binary)
    common.save(output/'plan.json',{'case':args.case,'model':'gpt-6.1-terra','effort':'medium',
                                    'timeout_seconds':60,'max_turns':1,'fingerprints':fingerprints,
                                    'request':pilot_fixture.CASES[args.case]['request']})
    auth = Path(os.environ.get('CODEX_HOME',str(Path.home()/'.codex')))/'auth.json'
    if not auth.is_file():
        common.save(output/'result.json',{'status':'authentication_unavailable','turns_started':0})
        print(json.dumps({'directory':str(output),'status':'authentication_unavailable'}))
        return
    began = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='gennie-pilot-direct-') as temporary:
        root = Path(temporary)
        home, work = root/'home', root/'work'
        home.mkdir(mode=0o700)
        work.mkdir(mode=0o700)
        (home/'auth.json').symlink_to(auth)
        trace, last = root/'trace.jsonl', root/'last.txt'
        env = os.environ.copy()
        env['CODEX_HOME'] = str(home)
        for name in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_DATABASE_PATH'):
            env.pop(name,None)
        config = [f'mcp_servers.gennie.command={json.dumps(sys.executable)}',
                  f'mcp_servers.gennie.args={json.dumps([str(server)])}',
                  f'mcp_servers.gennie.env.GENNIE_PILOT_CASE={json.dumps(args.case)}',
                  f'mcp_servers.gennie.env.GENNIE_PILOT_TRACE={json.dumps(str(trace))}',
                  'features.shell_tool=false','features.unified_exec=false','features.computer_use=false',
                  'features.browser_use=false','features.browser_use_external=false','features.multi_agent=false',
                  'features.plugins=false','features.apps=false','web_search="disabled"','model_reasoning_effort="medium"']
        if env.get('GENNIE_PILOT_ORIGIN'):
            config.append('mcp_servers.gennie.env.GENNIE_PILOT_ORIGIN='+json.dumps(env['GENNIE_PILOT_ORIGIN']))
        command = [str(binary),'exec','--ignore-user-config','--ephemeral','--skip-git-repo-check',
                   '--json','-s','read-only','-m','gpt-6.1-terra','-C',str(work),'-o',str(last)]
        for item in config:
            command.extend(['-c',item])
        command.append(pilot_fixture.CASES[args.case]['request'])
        process = subprocess.Popen(command,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
        try:
            stdout,_ = process.communicate(timeout=60)
            code = process.returncode
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            stdout,_ = process.communicate()
            code = None
        try:
            os.killpg(process.pid,0)
        except ProcessLookupError:
            clean = True
        else:
            os.killpg(process.pid,signal.SIGKILL)
            clean = False
        events = []
        for line in stdout.splitlines():
            try:
                item = json.loads(line)
                if isinstance(item,dict): events.append(item)
            except ValueError:
                pass
        calls = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
        candidates = [item['arguments']['recipe'] for item in calls if item.get('tool')=='test_recipe' and isinstance(item.get('arguments'),dict) and 'recipe' in item['arguments']]
        verified = pilot_fixture.verify(args.case,candidates[-1]) if candidates else None
        response = last.read_text() if last.exists() else None
        result = {'status':'completed' if code==0 and clean and response else 'technical_failure',
                  'quality':'completed' if verified and verified['passed'] else 'not_verified',
                  'verification':verified,'calls':calls,'response':response,'usage_reported':extract_usage(events),
                  'exit_code':code,'group_clean':clean,'elapsed_ms':round((time.monotonic()-began)*1000),'cost':None}
    result['temporary_removed'] = not root.exists()
    result['source_drift'] = any(common.sha_file(binary if key=='codex_binary' else ROOT/key) != value for key,value in fingerprints.items())
    common.save(output/'result.json',result)
    print(json.dumps({'directory':str(output),'status':result['status'],'quality':result['quality'],
                      'usage_reported':result['usage_reported'] is not None,'temporary_removed':result['temporary_removed']}))


if __name__ == '__main__':
    main()

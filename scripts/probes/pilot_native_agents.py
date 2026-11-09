#!/usr/bin/env python3
"""Celda experimental con delegación nativa Codex y Decisions directo."""
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
import pilot_codex_direct
import pilot_fixture
import pilot_native_mcp
import pilot_openai_decisions

ROOT = Path(__file__).resolve().parents[2]


def collect_session_metadata(home):
    """Metadatos y contadores de archivos propios, sin copiar conversaciones ni secretos."""
    sessions = []
    for path in (home/'sessions').rglob('*.jsonl'):
        meta, usage, types = {}, None, set()
        for line in path.read_text(encoding='utf-8').splitlines():
            try:
                item = json.loads(line)
            except ValueError:
                continue
            if not isinstance(item,dict):
                continue
            payload = item.get('payload') if isinstance(item.get('payload'),dict) else {}
            typ = payload.get('type')
            if isinstance(typ,str):
                types.add(typ)
            if item.get('type') == 'session_meta':
                for field in ('id','source','originator'):
                    if field in payload:
                        meta[field] = payload[field]
            if typ == 'token_count' and isinstance(payload.get('info'),dict):
                usage = payload['info'].get('total_token_usage')
        sessions.append({'meta':meta,'usage':usage,'event_types':sorted(types)})
    return sessions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', required=True, choices=tuple(pilot_fixture.CASES))
    args = parser.parse_args()
    output = ROOT/'.verification/pilot-native-agents'/str(uuid.uuid4())
    output.mkdir(parents=True, mode=0o700)
    binary = Path(common.codex_binary())
    server = Path(pilot_native_mcp.__file__).resolve()
    sources = [Path(__file__).resolve(), server, Path(pilot_fixture.__file__).resolve(),
               ROOT/'scripts/probes/pilot_openai_decisions.py']
    fingerprints = {str(p.relative_to(ROOT)):common.sha_file(p) for p in sources}
    fingerprints['codex_binary'] = common.sha_file(binary)
    common.save(output/'plan.json',{'case':args.case,'model':'gpt-6.1-terra','effort':'medium',
        'decision_model':'gpt-6-luna','timeout_seconds':180,'max_depth':2,
        'fingerprints':fingerprints,'request':pilot_fixture.CASES[args.case]['request']})
    auth = Path(os.environ.get('CODEX_HOME',str(Path.home()/'.codex')))/'auth.json'
    if not auth.is_file():
        common.save(output/'result.json',{'status':'authentication_unavailable'})
        return
    began = time.monotonic()
    decision_start = time.monotonic()
    try:
        decision = pilot_openai_decisions.choose(pilot_fixture.CASES[args.case]['request'],
            'Selecciona el ingeniero que corresponde a esta tarea.', ['web','desktop'])
    except pilot_openai_decisions.DecisionError as error:
        common.save(output/'result.json',{'status':'decision_failed','category':str(error),
            'elapsed_ms':round((time.monotonic()-began)*1000)})
        print(json.dumps({'directory':str(output),'status':'decision_failed','category':str(error)}))
        return
    decision['elapsed_ms'] = round((time.monotonic()-decision_start)*1000)
    if decision['choice'] != 'web':
        common.save(output/'result.json',{'status':'routing_failed','decision':decision,
            'elapsed_ms':round((time.monotonic()-began)*1000)})
        print(json.dumps({'directory':str(output),'status':'routing_failed'}))
        return
    with tempfile.TemporaryDirectory(prefix='gennie-pilot-native-') as temp:
        root = Path(temp)
        home, work = root/'home', root/'work'
        home.mkdir(mode=0o700)
        work.mkdir(mode=0o700)
        (home/'auth.json').symlink_to(auth)
        trace, last = root/'trace.jsonl', root/'last.txt'
        env = os.environ.copy()
        env['CODEX_HOME'] = str(home)
        for name in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_DATABASE_PATH'):
            env.pop(name,None)
        configs = [f'mcp_servers.gennie.command={json.dumps(sys.executable)}',
                   f'mcp_servers.gennie.args={json.dumps([str(server)])}',
                   f'mcp_servers.gennie.env.GENNIE_PILOT_CASE={json.dumps(args.case)}',
                   f'mcp_servers.gennie.env.GENNIE_PILOT_TRACE={json.dumps(str(trace))}',
                   'features.shell_tool=false','features.unified_exec=false','features.computer_use=false',
                   'features.browser_use=false','features.browser_use_external=false','features.multi_agent=true',
                   'features.plugins=false','features.apps=false','web_search="disabled"',
                   'model_reasoning_effort="medium"','agents.max_depth=2','agents.max_threads=3']
        command = [str(binary),'exec','--ignore-user-config','--skip-git-repo-check',
                   '--json','-s','read-only','-m','gpt-6.1-terra','-C',str(work),'-o',str(last)]
        for config in configs:
            command.extend(['-c',config])
        command.append('Piloto autorizado con datos sintéticos. Eres el arquitecto jefe. '
            'OpenAI Decisions seleccionó previamente al ingeniero web. '
            'Después lanza un subagente ingeniero web real usando la herramienta de delegación. '
            'Indícale investigar con inspect_page, formular al menos dos propuestas, '
            'lanzar un subagente ejecutor real para implementar y verificar UNA propuesta mediante test_recipe, '
            'esperar su resultado y validar personalmente la receta con test_recipe antes de reportártela. '
            'Si falla, debe diagnosticar con errores anteriores, evaluar el estancamiento y reintentar '
            'con contexto o cambiar propuesta, hasta un máximo de cuatro intentos. Nunca inventar éxito. '
            'El ejecutor devuelve receta y veredicto a su jefe inmediato. Tú esperas al ingeniero y presentas '
            'el resultado, plan, validaciones, errores y jerarquía observada. La tarea es: '+pilot_fixture.CASES[args.case]['request'])
        process = subprocess.Popen(command,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
        try:
            stdout,stderr = process.communicate(timeout=180)
            code = process.returncode
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            stdout,stderr = process.communicate()
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
                event = json.loads(line)
                if isinstance(event,dict): events.append(event)
            except ValueError:
                continue
        calls = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
        candidates = [x['arguments']['recipe'] for x in calls if x.get('tool')=='test_recipe' and isinstance(x.get('arguments'),dict) and 'recipe' in x['arguments']]
        verification = pilot_fixture.verify(args.case,candidates[-1]) if candidates else None
        decisions = [decision]
        common.save(output/'events.json',events)
        result = {'status':'completed' if code == 0 and clean and last.exists() else 'technical_failure',
            'quality':'completed' if verification and verification['passed'] else 'not_verified',
            'verification':verification,'calls':calls,'decisions':decisions,'decision_usage':[x.get('usage') for x in decisions],
            'codex_root_usage':pilot_codex_direct.extract_usage(events),
            'codex_all_agents_usage':None,'response':last.read_text() if last.exists() else None,
            'exit_code':code,'group_clean':clean,'elapsed_ms':round((time.monotonic()-began)*1000),
            'stderr_present':bool(stderr),'cost':None}
        result['session_metadata'] = collect_session_metadata(home)
    result['temporary_removed'] = not root.exists()
    result['source_drift'] = any(common.sha_file(binary if key=='codex_binary' else ROOT/key)!=value for key,value in fingerprints.items())
    common.save(output/'result.json',result)
    print(json.dumps({'directory':str(output),'status':result['status'],'quality':result['quality'],
                      'elapsed_ms':result['elapsed_ms'],'root_usage':result['codex_root_usage'],
                      'decision_calls':len(decisions),'temporary_removed':result['temporary_removed']}))


if __name__ == '__main__':
    main()

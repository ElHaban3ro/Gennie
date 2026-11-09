#!/usr/bin/env python3
"""Ejecuta el constructor real de Gennie sobre un sitio local temporal."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
import time
import uuid

import assignment_evaluation as common
import pilot_fixture
import pilot_web_fixture

ROOT = Path(__file__).resolve().parents[2]


def quality(case, builder):
    if not isinstance(builder,dict) or builder.get('state') != 'completed':
        return {'passed':False,'reason':'builder_not_completed'}
    output=builder.get('output')
    if not isinstance(output,dict) or output.get('status')!='ok':
        return {'passed':False,'reason':'plugin_not_executed'}
    expected=pilot_fixture.CASES[case]['expected']
    actual=output.get('data')
    return {'passed':actual==expected,'reason':'matched' if actual==expected else 'output_mismatch',
            'expected_fields':list(expected),'actual':actual}


def settings(db,profile_id='plugin_builder'):
    with sqlite3.connect('file:'+str(db.resolve())+'?mode=ro',uri=True) as connection:
        connection.execute('PRAGMA query_only=ON')
        row=connection.execute('''SELECT a.model,a.effort,p.personality,p.instructions,
            a.read_files,a.write_files,a.run_commands,a.local_actions
            FROM llm_architects a JOIN llm_architect_prompts p ON p.architect_id=a.id
            WHERE a.id=?''',(profile_id,)).fetchone()
    if row is None:
        raise RuntimeError('builder_profile_unavailable')
    model,effort,personality,instructions,read,write,commands,actions=row
    return {'model':model,'effort':effort,'personality':personality,'instructions':instructions,
            'read_files':bool(read),'write_files':bool(write),'run_commands':bool(commands),'local_actions':bool(actions)}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case',required=True,choices=tuple(pilot_fixture.CASES))
    parser.add_argument('--database',type=Path,default=ROOT/'data/soc.sqlite3')
    parser.add_argument('--names',action='store_true')
    args=parser.parse_args()
    profile=settings(args.database)
    output=ROOT/'.verification/pilot-gennie-real'/str(uuid.uuid4())
    output.mkdir(parents=True,mode=0o700)
    common.save(output/'plan.json',{'case':args.case,'profile':'plugin_builder','model':profile['model'],
        'effort':profile['effort'],'profile_source':'checkout_read_only','timeout_seconds':180,
        'profile_sha256':hashlib.sha256(json.dumps(profile,sort_keys=True,ensure_ascii=False).encode()).hexdigest(),
        'fixture':'local_loopback','personal_browser':False})
    result={'case':args.case,'status':'not_started'}
    with tempfile.TemporaryDirectory(prefix='gennie-pilot-result-') as temporary:
        result_path=Path(temporary)/'result.json'
        settings_path=Path(temporary)/'settings.json'
        settings_path.write_text(json.dumps(profile,ensure_ascii=False))
        settings_path.chmod(0o600)
        with pilot_web_fixture.serve(args.case) as site:
            env=os.environ.copy()
            for name in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY'):
                env.pop(name,None)
            env.update(GENNIE_PILOT_ORIGIN=site['origin'],GENNIE_PILOT_CASE=args.case,
                       GENNIE_PILOT_SETTINGS_FILE=str(settings_path),GENNIE_PILOT_OUTPUT=str(result_path))
            if args.names:
                env['GENNIE_PILOT_NAMES']='1'
            command=['cargo','test','-p','gennie-plugins','--locked','real_builder_runs_comparison_fixture',
                     '--','--ignored','--exact','--nocapture']
            # El nombre incluye el módulo; el filtro parcial es necesario.
            command.remove('--exact')
            started=time.monotonic()
            process=subprocess.Popen(command,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
            try:
                stdout,stderr=process.communicate(timeout=180)
                code=process.returncode
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL)
                stdout,stderr=process.communicate()
                code=None
            diagnostic = None
            lines = (stdout+b'\n'+stderr).decode('utf-8',errors='replace').splitlines()
            for index,line in enumerate(lines):
                if 'panicked at' in line:
                    diagnostic = lines[index+1][:500] if index+1 < len(lines) else 'panic_without_detail'
                    for sensitive in (profile['personality'],profile['instructions']):
                        if sensitive:
                            diagnostic=diagnostic.replace(sensitive,'[redacted]')
                    break
            result={'case':args.case,'exit_code':code,'elapsed_ms':round((time.monotonic()-started)*1000),
                    'status':'result_recorded' if result_path.is_file() else 'technical_failure',
                    'accesses':site['accesses'],'stdout_present':bool(stdout),'stderr_present':bool(stderr),
                    'diagnostic':diagnostic}
            if result_path.is_file():
                result['builder']=json.loads(result_path.read_text())
                if args.names:
                    browser=result['builder'].get('output') or {}
                    result['quality']={'passed':result['builder'].get('state')=='completed' and browser.get('status')=='ok' and browser.get('data')==['Ana','Luis']}
                else:
                    result['quality']=quality(args.case,result['builder'])
    common.save(output/'result.json',result)
    print(json.dumps({'directory':str(output),'status':result['status'],'exit_code':result['exit_code'],
                      'elapsed_ms':result['elapsed_ms']}))


if __name__ == '__main__':
    main()

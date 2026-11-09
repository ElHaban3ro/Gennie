#!/usr/bin/env python3
"""Una celda Codex directo o multiagente contra el sitio HTTP sintético."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import uuid

import assignment_evaluation as common
import pilot_fixture
import pilot_real_browser
import pilot_web_fixture

ROOT=Path(__file__).resolve().parents[2]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case',required=True,choices=tuple(pilot_fixture.CASES))
    parser.add_argument('--arm',required=True,choices=('direct','native'))
    parser.add_argument('--faults',type=int,choices=(0,1,2),default=0)
    args=parser.parse_args()
    directory=ROOT/'.verification/pilot-web-comparison'/str(uuid.uuid4())
    directory.mkdir(parents=True,mode=0o700)
    script='pilot_codex_direct.py' if args.arm=='direct' else 'pilot_native_goal.py'
    sources=[Path(__file__).resolve(),ROOT/'scripts/probes'/script,
             Path(pilot_web_fixture.__file__).resolve(),Path(pilot_real_browser.__file__).resolve(),
             ROOT/'apps/plugins/src/integration_tests.rs']
    fingerprints={str(path.relative_to(ROOT)):common.sha_file(path) for path in sources}
    fingerprints['codex_binary']=common.sha_file(Path(common.codex_binary()))
    common.save(directory/'plan.json',{'case':args.case,'arm':args.arm,'faults':args.faults,
                                      'external_browser_validation':True,'fingerprints':fingerprints})
    command=[sys.executable,str(ROOT/'scripts/probes'/script),'--case',args.case]
    if args.arm=='native':
        command.extend(['--mode','live','--native-v1'])
        if args.faults:
            command.extend(['--faults',str(args.faults)])
    elif args.faults:
        parser.error('--faults solo está disponible para native')
    with pilot_web_fixture.serve(args.case) as site:
        env=os.environ.copy()
        env['GENNIE_PILOT_ORIGIN']=site['origin']
        started=time.monotonic()
        process=subprocess.Popen(command,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                                 text=True,start_new_session=True)
        try:
            stdout,stderr=process.communicate(timeout=210)
            code=process.returncode
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            stdout,stderr=process.communicate()
            code=None
        try:
            result=json.loads(stdout.strip().splitlines()[-1])
        except (ValueError,IndexError):
            result=None
        browser_check=None
        if isinstance(result,dict) and result.get('directory'):
            detail_path=Path(result['directory'])/'result.json'
            if detail_path.is_file():
                detail=json.loads(detail_path.read_text())
                calls=detail.get('calls',[])
                recipes=[entry.get('arguments',{}).get('recipe') for entry in calls
                         if entry.get('tool')=='test_recipe' and isinstance(entry.get('result'),dict)
                         and entry['result'].get('passed') is True]
                if recipes:
                    browser_check=pilot_real_browser.check(args.case,recipes[-1],site['origin'])
        report={'case':args.case,'arm':args.arm,'exit_code':code,'result':result,
                'elapsed_ms':round((time.monotonic()-started)*1000),
                'accesses':site['accesses'],'stderr_present':bool(stderr),'browser_check':browser_check}
        report['source_drift']=any(common.sha_file(Path(common.codex_binary()) if key=='codex_binary' else ROOT/key)!=digest
                                   for key,digest in fingerprints.items())
    common.save(directory/'result.json',report)
    print(json.dumps({'directory':str(directory),'arm':args.arm,'result':result,'exit_code':code}))


if __name__=='__main__':
    main()

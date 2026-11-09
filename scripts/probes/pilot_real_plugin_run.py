#!/usr/bin/env python3
"""Una tarea real de plugin sobre Chromium y sitio sintético aislados."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import uuid

import assignment_evaluation as common
import pilot_web_fixture

ROOT=Path(__file__).resolve().parents[2]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--arm',required=True,choices=('direct','native'))
    args=parser.parse_args()
    directory=ROOT/'.verification/pilot-real-plugin'/str(uuid.uuid4())
    directory.mkdir(parents=True,mode=0o700)
    with tempfile.TemporaryDirectory(prefix='gennie-real-plugin-result-') as temp:
        output=Path(temp)/'result.json'
        with pilot_web_fixture.serve('simple') as site:
            env=os.environ.copy()
            for name in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY'):
                env.pop(name,None)
            env.update(GENNIE_REAL_ARM=args.arm,GENNIE_PILOT_ORIGIN=site['origin'],GENNIE_PILOT_OUTPUT=str(output))
            started=time.monotonic()
            process=subprocess.Popen(['cargo','test','-p','gennie-plugins','--locked','real_plugin_agent_comparison',
                                      '--','--ignored','--nocapture'],cwd=ROOT,env=env,
                                     stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
            try:
                stdout,stderr=process.communicate(timeout=210);code=process.returncode
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL);stdout,stderr=process.communicate();code=None
            lines=(stdout+b'\n'+stderr).decode('utf-8',errors='replace').splitlines()
            diagnostic=next((lines[i+1][:500] for i,line in enumerate(lines) if 'panicked at' in line and i+1<len(lines)),None)
            detail=json.loads(output.read_text()) if output.is_file() else None
            execution=detail.get('result') if isinstance(detail,dict) else None
            quality=isinstance(execution,dict) and execution.get('status')=='ok' and execution.get('executed') is True and execution.get('data')==['Ana','Luis']
            report={'arm':args.arm,'status':'result_recorded' if detail else 'technical_failure',
                'exit_code':code,'elapsed_ms':round((time.monotonic()-started)*1000),'quality':quality,
                'detail':detail,'diagnostic':diagnostic,'accesses':site['accesses']}
    common.save(directory/'result.json',report)
    print(json.dumps({'directory':str(directory),'status':report['status'],'quality':quality,'exit_code':code}))


if __name__=='__main__':
    main()

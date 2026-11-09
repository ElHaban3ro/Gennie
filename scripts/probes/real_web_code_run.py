#!/usr/bin/env python3
"""Celda comparativa: código JavaScript generado y sitio público en vivo."""
import argparse
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
import real_web_code_sandbox
import real_web_code_agent
import real_web_oracle
import real_web_read_proxy
import pilot_gennie_real

ROOT=Path(__file__).resolve().parents[2]


def digest(value):
    return hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True).encode()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--arm',required=True,choices=('gennie','direct','native'))
    args=parser.parse_args()
    directory=ROOT/'.verification/real-web-code'/str(uuid.uuid4());directory.mkdir(parents=True,mode=0o700)
    with tempfile.TemporaryDirectory(prefix='grw-code-',dir='/tmp') as temp:
        root=Path(temp);oracle=root/'oracle.json';trace=root/'trace.jsonl';agent=root/'agent.json'
        with real_web_read_proxy.serve() as proxy:
            expected=real_web_oracle.read(proxy['port']);oracle.write_text(json.dumps(expected,ensure_ascii=False));oracle.chmod(0o600)
            snapshot=digest(expected)
            env=os.environ.copy()
            for name in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY'):
                env.pop(name,None)
            env.update(GENNIE_REAL_WEB_ARM=args.arm,GENNIE_REAL_WEB_AGENT_OUTPUT=str(agent),
                GENNIE_REAL_WEB_PROXY_PORT=str(proxy['port']),GENNIE_REAL_WEB_TRACE=str(trace),GENNIE_REAL_WEB_ORACLE=str(oracle))
            if args.arm=='gennie':
                profile=pilot_gennie_real.settings(ROOT/'data/soc.sqlite3','gennie')
                settings_file=root/'settings.json';settings_file.write_text(json.dumps(profile,ensure_ascii=False));settings_file.chmod(0o600)
                prompt_file=root/'prompt.txt';prompt_file.write_text(real_web_code_agent.OBJECTIVE)
                env.update(GENNIE_REAL_WEB_SETTINGS=str(settings_file),GENNIE_REAL_WEB_PROMPT=str(prompt_file))
                command=['cargo','test','-p','gennie-llm','--locked','real_web_code_gennie_conversation','--','--ignored','--nocapture']
            else:
                command=[sys.executable,str(ROOT/'scripts/probes/real_web_code_agent.py')]
            started=time.monotonic()
            process=subprocess.Popen(command,cwd=ROOT,env=env,
                                     stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
            try:stdout,stderr=process.communicate(timeout=210);code=process.returncode
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL);stdout,stderr=process.communicate();code=None
            elapsed=round((time.monotonic()-started)*1000)
            report=json.loads(agent.read_text()) if agent.is_file() else {'status':'agent_result_missing'}
            if args.arm=='gennie' and isinstance(report.get('agent'),dict):
                report=report['agent']
            calls=[json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
            verified=[x for x in calls if x.get('tool')=='run_javascript' and x.get('result',{}).get('passed') is True]
            validation=None
            if verified:
                validation=real_web_code_sandbox.execute(verified[-1]['arguments']['source'],proxy['port'])
                validation={'exit_code':validation['exit_code'],'elapsed_ms':validation['elapsed_ms'],
                            'passed':validation['exit_code']==0 and validation['output']==expected,
                            'rows':len(validation['output']) if isinstance(validation['output'],list) else None}
            try:after=digest(real_web_oracle.read(proxy['port']))
            except (OSError,ValueError):after=None
            result={'arm':args.arm,'exit_code':code,'agent_elapsed_ms':elapsed,'agent':report,
                    'calls':calls,'independent_validation':validation,'site_snapshot_sha256':snapshot,
                    'site_snapshot_unchanged':after==snapshot if after else None,'accesses':proxy['accesses'],
                    'stderr_present':bool(stderr)}
    common.save(directory/'result.json',result)
    print(json.dumps({'directory':str(directory),'status':report['status'],'validated':bool(validation and validation['passed']),
                      'elapsed_ms':elapsed,'exit_code':code}))


if __name__=='__main__':main()

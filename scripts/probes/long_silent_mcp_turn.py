#!/usr/bin/env python3
"""Ejecuta una vez la sonda real de turno con herramienta MCP silenciosa."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import uuid

import pilot_gennie_real

ROOT = Path(__file__).resolve().parents[2]


def main():
    directory = ROOT/'.verification/long-silent-turn'/str(uuid.uuid4())
    directory.mkdir(parents=True, mode=0o700)
    profile = pilot_gennie_real.settings(ROOT/'data/soc.sqlite3', 'gennie')
    with tempfile.TemporaryDirectory(prefix='gennie-long-turn-run-', dir='/tmp') as tmp:
        root=Path(tmp); settings=root/'settings.json'; output=root/'output.json'
        settings.write_text(json.dumps(profile,ensure_ascii=False));settings.chmod(0o600)
        env=os.environ.copy()
        for key in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY'):
            env.pop(key,None)
        env.update(GENNIE_LONG_TURN_OUTPUT=str(output),GENNIE_LONG_TURN_SETTINGS=str(settings))
        started=time.monotonic()
        process=subprocess.Popen(['cargo','test','-p','gennie-llm','--locked','real_long_silent_mcp_turn',
            '--','--ignored','--nocapture'],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
        try:
            stdout,stderr=process.communicate(timeout=275); code=process.returncode
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL);stdout,stderr=process.communicate();code=None
        report=json.loads(output.read_text()) if output.is_file() else {'status':'output_missing'}
        report.update(exit_code=code,runner_elapsed_ms=round((time.monotonic()-started)*1000),
            stdout_present=bool(stdout),stderr_present=bool(stderr))
    path=directory/'result.json';path.write_text(json.dumps(report,ensure_ascii=False));path.chmod(0o600)
    print(json.dumps({'directory':str(directory),'status':report['status'],'exit_code':code,
        'calls':report.get('calls'),'tool_elapsed_ms':report.get('tool_elapsed_ms'),
        'elapsed_ms':report['runner_elapsed_ms']}))


if __name__ == '__main__': main()

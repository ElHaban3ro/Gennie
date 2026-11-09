#!/usr/bin/env python3
"""Convierte recetas verificadas en un plugin temporal y lo ejecuta en Chromium de Gennie."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
from urllib.parse import urlsplit

import pilot_fixture

ROOT=Path(__file__).resolve().parents[2]


def quality(case,result):
    if not isinstance(result,dict):
        return False
    verified=result.get('verify')
    executed=result.get('run')
    if not isinstance(verified,dict) or not isinstance(executed,dict):
        return False
    return (verified.get('status')=='ok' and verified.get('executed') is True
            and executed.get('status')=='ok' and executed.get('executed') is True
            and executed.get('data')==pilot_fixture.CASES[case]['expected'])


def manifest(case,recipe,origin):
    # El convertidor solo admite recetas verificadas por el parser limitado del banco.
    parsed=urlsplit(origin)
    if parsed.scheme!='http' or parsed.hostname!='127.0.0.1' or not parsed.port or parsed.path not in ('','/') or parsed.query or parsed.fragment or parsed.username or parsed.password:
        raise ValueError('invalid_fixture_origin')
    if not pilot_fixture.verify(case,recipe)['passed']:
        raise ValueError('unverified_recipe')
    statements=['await page.goto(origin + '+json.dumps('/'+recipe['source'])+');','const output = {};']
    for name,spec in recipe['fields'].items():
        statements.append('output['+json.dumps(name)+'] = await page.locator('+json.dumps(spec['selector'])+
          ').evaluateAll((nodes, attr) => nodes.map(node => node.getAttribute(attr)), '+json.dumps(spec['attribute'])+');')
    statements.append('return output;')
    return {'id':'fixture','name':'Comparación local','origin':origin,'functions':[
        {'name':'extract','description':'Extrae datos del directorio sintético','parameters':{},
         'transport':'program','path':'/','result_type':'object','code':'\n'.join(statements)}]}


def check(case,recipe,origin,timeout=90):
    value=manifest(case,recipe,origin)
    start=time.monotonic()
    with tempfile.TemporaryDirectory(prefix='gennie-pilot-browser-check-') as directory:
        root=Path(directory)
        manifest_path=root/'manifest.json'
        result_path=root/'result.json'
        manifest_path.write_text(json.dumps(value,ensure_ascii=False))
        env=os.environ.copy()
        for name in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY'):
            env.pop(name,None)
        env.update(GENNIE_PILOT_MANIFEST_FILE=str(manifest_path),GENNIE_PILOT_OUTPUT=str(result_path))
        process=subprocess.Popen(['cargo','test','-p','gennie-plugins','--locked',
            'real_browser_checks_comparison_manifest','--','--ignored','--nocapture'],
            cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
        try:
            process.communicate(timeout=timeout)
            code=process.returncode
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            process.communicate()
            code=None
        result=json.loads(result_path.read_text()) if result_path.is_file() else None
    data=(result.get('run') or {}).get('data') if isinstance(result,dict) else None
    return {'status':'completed' if code==0 and result else 'technical_failure','exit_code':code,
            'quality':quality(case,result),'data':data,
            'browser_result':result,'elapsed_ms':round((time.monotonic()-start)*1000)}

#!/usr/bin/env python3
"""MCP de lectura y ejecución JavaScript aislada para dos páginas web reales."""
import json
import os
from pathlib import Path
import sys
import time
from urllib.request import urlopen

import pilot_native_mcp
import real_web_code_sandbox

TOOLS=[
    {'name':'inspect_public_page','description':'Lee el HTML real de la página pública 1 o 2 del sitio autorizado.',
     'inputSchema':{'type':'object','properties':{'page':{'type':'integer','enum':[1,2]}},'required':['page'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
    {'name':'run_javascript','description':'Ejecuta JavaScript en Deno con acceso de solo lectura a las dos páginas autorizadas y compara su JSON con un oráculo independiente. Máximo cuatro ejecuciones por hilo.',
     'inputSchema':{'type':'object','properties':{'source':{'type':'string','minLength':1,'maxLength':32000}},'required':['source'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
]


def run_tool(name,args,port,trace,metadata,expected):
    if name=='inspect_public_page' and set(args)=={'page'} and type(args['page']) is int and args['page'] in (1,2):
        with urlopen(f'http://127.0.0.1:{port}/page/{args["page"]}',timeout=12) as response:
            body=response.read(100_001)
        if len(body)>100_000:raise ValueError('page_too_large')
        return {'status':'ok','html':body.decode('utf-8')}
    if name=='run_javascript' and set(args)=={'source'}:
        error=pilot_native_mcp.reserve_recipe(trace,metadata)
        if error:return {'status':error,'passed':False}
        if os.environ.get('GENNIE_EXTENDED_TRANSIENT') == '1':
            try:
                fd = os.open(trace + '.transient', os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
            except FileExistsError:
                pass
            else:
                os.close(fd)
                return {'status':'transient_unavailable','passed':False,'error':'Servicio temporalmente no disponible; vuelve a intentar.'}
        result=real_web_code_sandbox.execute(args['source'],port)
        actual=result.get('output')
        passed=result['exit_code']==0 and actual==expected
        robustness = None
        if passed and os.environ.get('GENNIE_LONG_ROBUST') == '1':
            import long_web_robustness
            robustness = long_web_robustness.check(args['source'], port, expected)
            passed = all(robustness.values())
        # El agente recibe un resumen de cobertura; el oráculo permanece oculto.
        return {'status':'verified' if passed else 'mismatch' if result['exit_code']==0 else 'execution_failed',
                'passed':passed,'rows':len(actual) if isinstance(actual,list) else None,
                'elapsed_ms':result['elapsed_ms'],'error':result.get('stderr_excerpt'),
                'robustness':robustness}
    raise ValueError('invalid_tool_or_arguments')


def main():
    port=int(os.environ['GENNIE_REAL_WEB_PROXY_PORT'])
    trace=os.environ['GENNIE_REAL_WEB_TRACE']
    expected=json.loads(Path(os.environ['GENNIE_REAL_WEB_ORACLE']).read_text())
    for line in sys.stdin:
        request=None
        try:
            request=json.loads(line)
            if not isinstance(request,dict) or 'id' not in request:continue
            method=request.get('method')
            if method=='initialize':
                result={'protocolVersion':'2025-03-26','capabilities':{'tools':{}},'serverInfo':{'name':'real-code','version':'1'}}
            elif method=='tools/list':result={'tools':TOOLS}
            elif method=='tools/call':
                params=request.get('params',{});name=params.get('name');args=params.get('arguments')
                if not isinstance(args,dict):raise ValueError('invalid_arguments')
                meta=params.get('_meta') if isinstance(params.get('_meta'),dict) else {}
                began=time.monotonic();value=run_tool(name,args,port,trace,meta,expected)
                entry={'tool':name,'threadId':meta.get('threadId'),'arguments':args,'result':value,
                       'elapsed_ms':round((time.monotonic()-began)*1000)}
                with open(trace,'a',encoding='utf-8') as f:f.write(json.dumps(entry,ensure_ascii=False)+'\n')
                result={'content':[{'type':'text','text':json.dumps(value,ensure_ascii=False)}], 'structuredContent':value}
            else:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32601,'message':'unknown method'}}),flush=True);continue
            print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result},ensure_ascii=False),flush=True)
        except (ValueError,TypeError,KeyError,OSError):
            if isinstance(request,dict) and 'id' in request:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32602,'message':'invalid request or fixture unavailable'}}),flush=True)


if __name__=='__main__':main()

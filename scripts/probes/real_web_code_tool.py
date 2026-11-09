#!/usr/bin/env python3
"""Puente puntual de las herramientas reales para la sesión de Gennie."""
import json
import os
from pathlib import Path
import sys
import time

import real_web_code_mcp


def main():
    request=json.load(sys.stdin)
    name=request['name'];args=request['arguments']
    port=int(os.environ['GENNIE_REAL_WEB_PROXY_PORT']);trace=os.environ['GENNIE_REAL_WEB_TRACE']
    expected=json.loads(Path(os.environ['GENNIE_REAL_WEB_ORACLE']).read_text())
    meta={'threadId':'00000000-0000-4000-8000-000000000001'}
    start=time.monotonic()
    try:result=real_web_code_mcp.run_tool(name,args,port,trace,meta,expected)
    except (ValueError,TypeError,KeyError,OSError):result={'status':'tool_failed','passed':False}
    with open(trace,'a',encoding='utf-8') as f:
        f.write(json.dumps({'tool':name,'threadId':'gennie-bot','arguments':args,'result':result,
                            'elapsed_ms':round((time.monotonic()-start)*1000)},ensure_ascii=False)+'\n')
    print(json.dumps(result,ensure_ascii=False))


if __name__=='__main__':main()

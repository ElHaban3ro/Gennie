#!/usr/bin/env python3
"""Servidor MCP de prueba: datos ficticios, registro sin argumentos y cero red."""
import json
from pathlib import Path
import sys

log = Path(sys.argv[1])
tool = {'name':'echo','description':'Consulta sintética; no opera servicios reales.',
        'inputSchema':{'type':'object','properties':{'message':{'type':'string','title':'Mensaje','minLength':1}},'required':['message'],'additionalProperties':False}}
for line in sys.stdin:
    request = json.loads(line)
    method = request.get('method')
    if 'id' not in request:
        continue
    error = None
    if method == 'initialize':
        result = {'protocolVersion':request['params']['protocolVersion'],'capabilities':{'tools':{}},'serverInfo':{'name':'Gennie fixture','version':'1'}}
    elif method == 'tools/list':
        result = {'tools':[tool]}
    elif method == 'tools/call':
        with log.open('a') as file:
            file.write(json.dumps({'tool':'echo'})+'\n')
        message = request['params']['arguments']['message']
        if message == 'desconectar':
            sys.exit(0)
        result = {'isError':False,'content':[{'type':'text','text':'Resultado ficticio: '+message}],'structuredContent':{'echo':message}}
    elif method == 'ping':
        result = {}
    else:
        error = {'code':-32601,'message':'Método no disponible en el fixture'}
    response = {'jsonrpc':'2.0','id':request['id']}
    response['error' if error else 'result'] = error or result
    print(json.dumps(response), flush=True)

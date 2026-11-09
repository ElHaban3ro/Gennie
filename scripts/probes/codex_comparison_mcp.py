#!/usr/bin/env python3
"""MCP stdio de lectura ficticia para comparar Codex directo, sin servicios externos."""
import json
import os
import sys

DESCRIPTION = ('Lee una fuente ficticia de la evaluación. Targets disponibles: alpha y beta para el primer caso, '
               'primary y backup para el segundo. No accede a cuentas ni ejecuta efectos.')
SCHEMA = {'type':'object','properties':{'target':{'type':'string','enum':['alpha','beta','primary','backup']}},
          'required':['target'],'additionalProperties':False}


def main():
    trace_path = os.environ['CONTINUITY_COMPARISON_TRACE']
    def record(event):
        with open(trace_path,'a',encoding='utf-8') as stream:
            stream.write(json.dumps(event,ensure_ascii=False)+'\n')
    for line in sys.stdin:
        try:
            request = json.loads(line)
            method = request.get('method')
            if 'id' not in request:
                continue
            if method == 'initialize':
                record({'event':'mcp_initialize'})
                result = {'protocolVersion':'2025-03-26','capabilities':{'tools':{}},
                          'serverInfo':{'name':'fixture','version':'1'}}
            elif method == 'tools/list':
                record({'event':'mcp_tools_list'})
                result = {'tools':[{'name':'probe_read','description':DESCRIPTION,'inputSchema':SCHEMA,
                                    'annotations':{'readOnlyHint':True}}]}
            elif method == 'tools/call':
                args = request.get('params',{}).get('arguments',{})
                target = args.get('target')
                if request.get('params',{}).get('name') != 'probe_read' or target not in ('alpha','beta'):
                    raise ValueError('solicitud ficticia no admitida')
                value = {'alpha':'LIMA-42','beta':'SOL-71'}[target]
                for event in ({'event':'call','target':target},{'event':'observation','target':target,'value':value}):
                    record(event)
                result = {'content':[{'type':'text','text':json.dumps({'status':'ok','value':value})}],
                          'structuredContent':{'status':'ok','value':value}}
            else:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32601,'message':'Método no disponible'}}),flush=True)
                continue
            print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result},ensure_ascii=False),flush=True)
        except (ValueError, TypeError, KeyError, json.JSONDecodeError):
            if isinstance(locals().get('request'),dict) and 'id' in request:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32602,'message':'Solicitud inválida'}}),flush=True)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Herramientas MCP ficticias para el piloto; solo leen HTML y prueban recetas locales."""
import json
import os
import sys
import pilot_fixture
import pilot_web_fixture

TOOLS = [
    {'name':'inspect_page','description':'Lee el HTML de una página ficticia por source: main o backup.',
     'inputSchema':{'type':'object','properties':{'source':{'type':'string','enum':['main','backup']}},
                    'required':['source'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
    {'name':'test_recipe','description':'Ejecuta una receta declarativa contra la página ficticia. La receta usa source y fields; cada field contiene selector CSS limitado a tag.clase y attribute. Devuelve datos extraídos y si cubre el resultado pedido.',
     'inputSchema':{'type':'object','properties':{'recipe':{'type':'object','properties':{
         'source':{'type':'string','enum':['main','backup']},
         'fields':{'type':'object','additionalProperties':{'type':'object','properties':{
             'selector':{'type':'string'},'attribute':{'type':'string'}},'required':['selector','attribute'],'additionalProperties':False}}},
         'required':['source','fields'],'additionalProperties':False}},'required':['recipe'],'additionalProperties':False},
     'annotations':{'readOnlyHint':True}},
]


def invoke(case, name, args):
    if not isinstance(args,dict):
        raise ValueError('arguments_invalid')
    if name == 'inspect_page' and set(args) == {'source'}:
        if os.environ.get('GENNIE_PILOT_ORIGIN'):
            return pilot_web_fixture.inspect(os.environ['GENNIE_PILOT_ORIGIN'],args['source'])
        return pilot_fixture.inspect(case,args['source'])
    if name == 'test_recipe' and set(args) == {'recipe'}:
        return pilot_fixture.verify(case,args['recipe'])
    raise ValueError('tool_or_arguments_invalid')


def main():
    case = os.environ['GENNIE_PILOT_CASE']
    trace = os.environ['GENNIE_PILOT_TRACE']
    if case not in pilot_fixture.CASES:
        raise SystemExit('invalid case')
    for line in sys.stdin:
        request = None
        try:
            request = json.loads(line)
            if not isinstance(request,dict) or 'id' not in request:
                continue
            method = request.get('method')
            if method == 'initialize':
                result = {'protocolVersion':'2025-03-26','capabilities':{'tools':{}},'serverInfo':{'name':'pilot','version':'1'}}
            elif method == 'tools/list':
                result = {'tools':TOOLS}
            elif method == 'tools/call':
                params = request.get('params',{})
                name, args = params.get('name'), params.get('arguments')
                value = invoke(case,name,args)
                with open(trace,'a',encoding='utf-8') as stream:
                    stream.write(json.dumps({'tool':name,'arguments':args,'result':value},ensure_ascii=False)+'\n')
                result = {'content':[{'type':'text','text':json.dumps(value,ensure_ascii=False)}],
                          'structuredContent':value}
            else:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32601,'message':'unknown method'}}),flush=True)
                continue
            print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result},ensure_ascii=False),flush=True)
        except (ValueError, TypeError, KeyError, json.JSONDecodeError):
            if isinstance(request,dict) and 'id' in request:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32602,'message':'invalid request'}}),flush=True)


if __name__ == '__main__':
    main()

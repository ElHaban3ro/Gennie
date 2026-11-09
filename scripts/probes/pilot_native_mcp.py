#!/usr/bin/env python3
"""Herramientas del piloto nativo: fixture local y clasificación OpenAI Decisions."""
import json
import fcntl
import hashlib
import os
import sys
import time
import uuid

import pilot_fixture
import pilot_openai_decisions
import pilot_web_fixture

TOOLS = [
    {'name':'record_plan','description':'Registra dos o más propuestas distintas y errores previstos antes de probar recetas.',
     'inputSchema':{'type':'object','properties':{
       'proposals':{'type':'array','minItems':2,'maxItems':6,'items':{'type':'string','minLength':1,'maxLength':1000}},
       'static_errors':{'type':'array','maxItems':12,'items':{'type':'string','maxLength':500}},
       'dynamic_errors':{'type':'array','maxItems':12,'items':{'type':'string','maxLength':500}}},
       'required':['proposals','static_errors','dynamic_errors'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
    {'name': 'inspect_page', 'description': 'Inspecciona una página sintética: main o backup.',
     'inputSchema': {'type':'object','properties':{'source':{'type':'string','enum':['main','backup']}},'required':['source'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
    {'name': 'test_recipe', 'description': 'Prueba una receta con source main o backup y fields por nombre; cada campo tiene selector limitado tag.clase y attribute.',
     'inputSchema': {'type':'object','properties':{'recipe':{'type':'object','properties':{
         'source':{'type':'string','enum':['main','backup']},
         'fields':{'type':'object','additionalProperties':{'type':'object','properties':{
             'selector':{'type':'string'},'attribute':{'type':'string'}},'required':['selector','attribute'],'additionalProperties':False}}},
         'required':['source','fields'],'additionalProperties':False}},'required':['recipe'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
    {'name':'decide','description':'Clasifica con OpenAI Decisions directo; envía la evidencia sintética indicada a OpenAI. Máximo seis consultas.',
     'inputSchema':{'type':'object','properties':{'evidence':{'type':'string'},'instructions':{'type':'string'},
       'choices':{'type':'array','items':{'type':'string'},'minItems':2,'maxItems':10}},
       'required':['evidence','instructions','choices'],'additionalProperties':False},'annotations':{'readOnlyHint':True}},
]


def reserve(trace, kind, maximum):
    """Límite compartido por todos los procesos MCP de una ejecución."""
    with open(trace+'.'+kind+'.budget','a+',encoding='utf-8') as budget:
        fcntl.flock(budget,fcntl.LOCK_EX)
        budget.seek(0)
        count = int(budget.read() or '0')
        if count >= maximum:
            return False
        budget.seek(0)
        budget.truncate()
        budget.write(str(count+1))
        budget.flush()
        return True


def reserve_recipe(trace, metadata):
    """El identificador lo proporciona el transporte; falta o formato inválido falla cerrado."""
    thread=metadata.get('threadId') if isinstance(metadata,dict) else None
    if not isinstance(thread,str):
        return 'thread_identity_unavailable'
    try:
        if str(uuid.UUID(thread)) != thread:
            return 'thread_identity_unavailable'
    except ValueError:
        return 'thread_identity_unavailable'
    digest=hashlib.sha256(thread.encode()).hexdigest()
    if not reserve(trace,'recipe_thread_'+digest,4):
        return 'executor_attempt_budget_exhausted'
    if not reserve(trace,'recipe_global',12):
        return 'recipe_global_budget_exhausted'
    return None


def invoke(case, name, args, decide=pilot_openai_decisions.choose):
    if not isinstance(args, dict):
        raise ValueError('invalid_arguments')
    if name == 'inspect_page' and set(args) == {'source'}:
        if os.environ.get('GENNIE_PILOT_ORIGIN'):
            return pilot_web_fixture.inspect(os.environ['GENNIE_PILOT_ORIGIN'],args['source'])
        return pilot_fixture.inspect(case,args['source'])
    if name == 'test_recipe' and set(args) == {'recipe'}:
        return pilot_fixture.verify(case,args['recipe'])
    if name == 'record_plan' and set(args) == {'proposals','static_errors','dynamic_errors'}:
        proposals=args['proposals']
        if not isinstance(proposals,list) or not 2 <= len(proposals) <= 6 or any(not isinstance(x,str) or not x.strip() or len(x)>1000 for x in proposals) or len({x.strip() for x in proposals}) != len(proposals):
            raise ValueError('invalid_proposals')
        if any(not isinstance(args[key],list) or len(args[key])>12 or any(not isinstance(x,str) or len(x)>500 for x in args[key]) for key in ('static_errors','dynamic_errors')):
            raise ValueError('invalid_errors')
        return {'status':'plan_recorded','proposal_count':len(proposals)}
    if name == 'decide' and set(args) == {'evidence','instructions','choices'}:
        if not isinstance(args['choices'],list):
            raise ValueError('invalid_choices')
        return decide(args['evidence'],args['instructions'],args['choices'])
    raise ValueError('invalid_tool')


def main():
    case, trace = os.environ['GENNIE_PILOT_CASE'], os.environ['GENNIE_PILOT_TRACE']
    if case not in pilot_fixture.CASES:
        raise SystemExit('invalid_case')
    for line in sys.stdin:
        request = None
        try:
            request = json.loads(line)
            if not isinstance(request,dict) or 'id' not in request:
                continue
            method = request.get('method')
            if method == 'initialize':
                result = {'protocolVersion':'2025-03-26','capabilities':{'tools':{}},'serverInfo':{'name':'pilot-native','version':'1'}}
            elif method == 'tools/list':
                exposed=[tool for tool in TOOLS if os.environ.get('GENNIE_PILOT_PLUGIN_MODE')!='1' or tool['name'] in ('record_plan','decide')]
                result = {'tools':exposed}
            elif method == 'tools/call':
                params = request.get('params',{})
                name, args = params.get('name'), params.get('arguments')
                metadata=params.get('_meta') if isinstance(params.get('_meta'),dict) else {}
                start = time.monotonic()
                try:
                    if name == 'decide':
                        if not reserve(trace,'decisions',6):
                            raise pilot_openai_decisions.DecisionError('decision_budget_exhausted')
                    if name == 'test_recipe' and not os.path.exists(trace+'.plan'):
                        value = {'status':'plan_required','passed':False}
                    elif name == 'test_recipe' and (budget_error:=reserve_recipe(trace,metadata)):
                        value = {'status':budget_error,'passed':False}
                    elif name == 'test_recipe' and os.environ.get('GENNIE_PILOT_RECIPE_FAULTS') in ('1','2') and reserve(trace,'injected_fault',int(os.environ['GENNIE_PILOT_RECIPE_FAULTS'])):
                        value = {'status':'injected_transient_failure','passed':False,'message':'Fallo temporal sintético de la prueba.'}
                    else:
                        value = invoke(case,name,args)
                        if name == 'record_plan' and value.get('status') == 'plan_recorded':
                            temporary_plan=trace+'.plan.'+str(os.getpid())
                            with open(temporary_plan,'w',encoding='utf-8') as plan:
                                json.dump(args,plan,ensure_ascii=False)
                            os.replace(temporary_plan,trace+'.plan')
                except pilot_openai_decisions.DecisionError as error:
                    value = {'status':'decision_failed','error':str(error)}
                record = {'tool':name,'arguments':args,'result':value,'elapsed_ms':round((time.monotonic()-start)*1000),
                          'meta_keys':sorted(metadata),'meta_thread_id':metadata.get('threadId') if isinstance(metadata.get('threadId'),str) else None}
                with open(trace,'a',encoding='utf-8') as stream:
                    stream.write(json.dumps(record,ensure_ascii=False)+'\n')
                result = {'content':[{'type':'text','text':json.dumps(value,ensure_ascii=False)}], 'structuredContent':value}
            else:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32601,'message':'unknown method'}}),flush=True)
                continue
            print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result},ensure_ascii=False),flush=True)
        except (ValueError,TypeError,KeyError):
            if isinstance(request,dict) and 'id' in request:
                print(json.dumps({'jsonrpc':'2.0','id':request['id'],'error':{'code':-32602,'message':'invalid request'}}),flush=True)


if __name__ == '__main__':
    main()

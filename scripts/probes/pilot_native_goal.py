#!/usr/bin/env python3
"""Sonda de Goal nativo con Codex app-server, MCP ficticio y Decisions directo."""
import argparse
from collections import Counter
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
import assignment_evaluation_worker as worker
import pilot_fixture
import pilot_native_agents
import pilot_native_mcp
import pilot_openai_decisions

ROOT = Path(__file__).resolve().parents[2]


def audit(report):
    """No convierte una receta correcta o un Goal raíz en éxito de toda la jerarquía."""
    failures=[]
    if report.get('goal_status') != 'complete':
        failures.append('root_goal_incomplete')
    if not isinstance(report.get('verification'),dict) or report['verification'].get('passed') is not True:
        failures.append('recipe_not_verified')
    executors=report.get('executor_goals') or {}
    updates=report.get('goal_updates_by_thread') or {}
    if not executors:
        failures.append('executor_not_observed')
    traces=[item for item in (report.get('calls') or []) if item.get('tool')=='test_recipe']
    events=report.get('recipe_evidence') or []
    def key(thread,recipe,passed):
        return (thread,json.dumps(recipe,sort_keys=True,ensure_ascii=False),passed)
    trace_keys=Counter(key(item.get('meta_thread_id'),item.get('arguments',{}).get('recipe'),item.get('result',{}).get('passed')) for item in traces)
    event_keys=Counter(key(item.get('threadId'),item.get('recipe'),item.get('passed')) for item in events)
    if not traces or trace_keys != event_keys:
        failures.append('recipe_attribution_incomplete')
    counts=Counter(item.get('meta_thread_id') for item in traces if item.get('result',{}).get('status') not in ('executor_attempt_budget_exhausted','recipe_global_budget_exhausted','thread_identity_unavailable','plan_required'))
    if any(thread is None or count>4 for thread,count in counts.items()):
        failures.append('recipe_budget_violation')
    for thread,entry in executors.items():
        if entry.get('status') != 'accepted' or updates.get(thread,{}).get('status') != 'complete':
            failures.append('executor_goal_incomplete')
        if thread not in (report.get('codex_usage_by_thread') or {}):
            failures.append('executor_usage_missing')
        parent=(report.get('agent_parents') or {}).get(thread)
        verified=[item for item in (report.get('recipe_evidence') or []) if item.get('passed') is True]
        child_recipes=[item.get('recipe') for item in verified if item.get('threadId')==thread]
        parent_recipes=[item.get('recipe') for item in verified if item.get('threadId')==parent]
        if not child_recipes:
            failures.append('executor_verification_missing')
        if not parent or not any(recipe in parent_recipes for recipe in child_recipes):
            failures.append('parent_verification_missing')
        plans=report.get('plan_evidence') or []
        if not any(plan.get('threadId')==parent and plan.get('status')=='plan_recorded' and plan.get('proposal_count',0)>=2 for plan in plans):
            failures.append('parent_plan_missing')
    return {'passed':not failures,'failures':failures}


def may_close_executor(child,parent,goal,observations,evidence):
    if not parent or goal.get('status')!='active':
        return False
    if not any(x.get('method')=='turn/completed' and x.get('threadId')==child for x in observations):
        return False
    verified=[x for x in evidence if x.get('passed') is True]
    own=[x.get('recipe') for x in verified if x.get('threadId')==child]
    reviewed=[x.get('recipe') for x in verified if x.get('threadId')==parent]
    return any(recipe is not None and recipe in reviewed for recipe in own)


class Client(worker.Client):
    def __init__(self, process, until, case=None):
        super().__init__(process,until)
        self.case = case
        self.thread_usage = {}
        self.observations = []
        self.executor_goals = {}
        self.agent_depths = {}
        self.agent_parents = {}
        self.recipe_evidence = []
        self.plan_evidence = []
        self.goal_updates = {}
        self.root_turns_started = 0

    def register_executor(self, child, path):
        if child in self.executor_goals:
            return
        identifier=100+len(self.executor_goals)
        self.executor_goals[child]={'request_id':identifier,'agent_path':path,'status':'requested'}
        self.send({'id':identifier,'method':'thread/goal/set','params':{'threadId':child,
          'origin':'user','objective':'Implementa y verifica la propuesta recibida sin añadir ni cambiar campos de la petición original: '+pilot_fixture.CASES[self.case]['request']+' Informa evidencia y errores a tu jefe inmediato.',
          'status':'active','tokenBudget':100000}})

    def observe(self, value):
        method, params = value.get('method'), value.get('params',{})
        if method == 'turn/started' and params.get('threadId') == self.thread_id:
            self.root_turns_started += 1
        if method == 'thread/goal/updated' and isinstance(params.get('goal'),dict):
            goal=params['goal']
            if isinstance(goal.get('threadId'),str):
                self.goal_updates[goal['threadId']]=goal
        if method == 'thread/tokenUsage/updated' and isinstance(params.get('threadId'),str):
            self.thread_usage[params['threadId']] = worker.usage_snapshot(params.get('tokenUsage'))
        if method in ('thread/started','thread/goal/updated','turn/started','turn/completed','item/started','item/completed'):
            item = params.get('item',{})
            if method=='item/completed' and isinstance(item,dict) and item.get('type')=='mcpToolCall' and item.get('tool')=='test_recipe':
                result=item.get('result') or {}
                result=result.get('structuredContent') if isinstance(result,dict) else None
                args=item.get('arguments') or {}
                self.recipe_evidence.append({'threadId':params.get('threadId'),
                    'recipe':args.get('recipe') if isinstance(args,dict) else None,
                    'passed':result.get('passed') if isinstance(result,dict) else None})
            if method=='item/completed' and isinstance(item,dict) and item.get('type')=='mcpToolCall' and item.get('tool')=='record_plan':
                result=item.get('result') or {}
                result=result.get('structuredContent') if isinstance(result,dict) else None
                self.plan_evidence.append({'threadId':params.get('threadId'),
                    'status':result.get('status') if isinstance(result,dict) else None,
                    'proposal_count':result.get('proposal_count') if isinstance(result,dict) else None})
            event={'method':method,'threadId':params.get('threadId'),
                'itemType':item.get('type') if isinstance(item,dict) else None}
            if isinstance(item,dict) and item.get('type') in ('subAgentActivity','collabAgentToolCall'):
                event['item']={k:item[k] for k in ('agentPath','agentThreadId','kind','senderThreadId',
                    'receiverThreadIds','tool','status') if k in item}
                path=item.get('agentPath')
                child=item.get('agentThreadId')
                if isinstance(path,str) and path.count('/') >= 3 and isinstance(child,str) and child not in self.executor_goals:
                    self.register_executor(child,path)
                if item.get('type') == 'collabAgentToolCall' and item.get('tool') == 'spawnAgent':
                    parent=item.get('senderThreadId')
                    depth=self.agent_depths.get(parent,0)+1
                    for spawned in item.get('receiverThreadIds',[]):
                        if isinstance(spawned,str):
                            self.agent_depths[spawned]=depth
                            self.agent_parents[spawned]=parent
                            if depth >= 2:
                                self.register_executor(spawned,'native-v1-depth-'+str(depth))
            self.observations.append(event)
        if isinstance(value.get('id'),int):
            for entry in self.executor_goals.values():
                if entry['request_id'] == value['id']:
                    entry['status'] = 'rejected' if 'error' in value else 'accepted'
                    if 'result' in value:
                        entry['goal'] = value['result'].get('goal')
                    elif 'error' in value:
                        entry['error_code'] = value['error'].get('code')
                        message = value['error'].get('message')
                        if isinstance(message,str):
                            entry['error_message'] = message[:500]
        super().observe(value)


def run(case, mode, native_v1=False, faults=0):
    output = ROOT/'.verification/pilot-native-goal'/str(uuid.uuid4())
    output.mkdir(parents=True,mode=0o700)
    start = time.monotonic()
    report = {'case':case,'mode':mode,'native_v1':native_v1,'injected_faults':faults,'status':'not_started','goal_status':None,'codex_usage_by_thread':{},'decision':None}
    paths=[Path(__file__).resolve(),Path(pilot_native_mcp.__file__).resolve(),
           Path(pilot_openai_decisions.__file__).resolve(),Path(pilot_fixture.__file__).resolve(),
           ROOT/'scripts/probes/pilot_web_fixture.py']
    fingerprints={str(p.relative_to(ROOT)):common.sha_file(p) for p in paths}
    fingerprints['codex_binary']=common.sha_file(Path(worker.binary()))
    common.save(output/'plan.json',{'case':case,'mode':mode,'native_v1':native_v1,'model':'gpt-6.1-terra',
        'effort':'medium','decision_model':'gpt-6-luna','max_recipe_tests_per_thread':4,'max_recipe_tests_global':12,'injected_faults':faults,
        'max_decisions':6,'max_root_turns':4,'deadline_seconds':180,'fingerprints':fingerprints})
    if mode == 'live':
        try:
            report['decision'] = pilot_openai_decisions.choose(pilot_fixture.CASES[case]['request'],
                'Elige el ingeniero adecuado para el problema.', ['web','desktop'])
        except pilot_openai_decisions.DecisionError as error:
            report.update(status='decision_failed',error=str(error))
            common.save(output/'result.json',report)
            return output,report
        if report['decision']['choice'] != 'web':
            report['status'] = 'routing_failed'
            common.save(output/'result.json',report)
            return output,report
    process = None
    with tempfile.TemporaryDirectory(prefix='gennie-goal-native-') as temporary:
        root = Path(temporary)
        home,work = root/'home',root/'work'
        home.mkdir(mode=0o700)
        work.mkdir(mode=0o700)
        if mode == 'live':
            auth = Path(os.environ.get('CODEX_HOME',str(Path.home()/'.codex')))/'auth.json'
            if not auth.is_file():
                report['status']='authentication_unavailable'
                common.save(output/'result.json',report)
                return output,report
            (home/'auth.json').symlink_to(auth)
        env = os.environ.copy()
        env['CODEX_HOME'] = str(home)
        for name in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_DATABASE_PATH'):
            env.pop(name,None)
        trace=root/'trace.jsonl'
        server=str(Path(pilot_native_mcp.__file__).resolve())
        configs = [f'mcp_servers.gennie.command={json.dumps(sys.executable)}',
            f'mcp_servers.gennie.args={json.dumps([server])}',
            f'mcp_servers.gennie.env.GENNIE_PILOT_CASE={json.dumps(case)}',
            f'mcp_servers.gennie.env.GENNIE_PILOT_TRACE={json.dumps(str(trace))}',
            'features.shell_tool=false','features.unified_exec=false','features.computer_use=false',
            'features.browser_use=false','features.browser_use_external=false','features.multi_agent=true',
            'features.plugins=false','features.apps=false','features.goals=true','web_search="disabled"',
            'model_reasoning_effort="medium"','agents.max_depth=2','agents.max_threads=3']
        if env.get('GENNIE_PILOT_ORIGIN'):
            configs.append('mcp_servers.gennie.env.GENNIE_PILOT_ORIGIN='+json.dumps(env['GENNIE_PILOT_ORIGIN']))
        if faults:
            configs.append('mcp_servers.gennie.env.GENNIE_PILOT_RECIPE_FAULTS='+json.dumps(str(faults)))
        if native_v1:
            catalog=json.loads((Path.home()/'.codex/models_cache.json').read_text())
            models=catalog.get('models',[])
            selected=[x for x in models if x.get('slug') == 'gpt-6.1-terra']
            if len(selected) != 1:
                raise RuntimeError('catalog_model_unavailable')
            selected[0]['multi_agent_version']='v1'
            temporary_catalog=root/'models.json'
            temporary_catalog.write_text(json.dumps({'models':models}))
            configs.extend(['model_catalog_json='+json.dumps(str(temporary_catalog)), 'features.multi_agent_v2=false'])
        command=[worker.binary(),'app-server','--listen','stdio://']
        for config in configs:
            command.extend(['-c',config])
        process=subprocess.Popen(command,cwd=work,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL,bufsize=0,start_new_session=True)
        client=Client(process,start+180,case)
        try:
            client.call(1,'initialize',{'clientInfo':{'name':'gennie_goal_experiment','version':'1'},'capabilities':{'experimentalApi':True}})
            client.send({'method':'initialized','params':{}})
            thread=client.call(2,'thread/start',{'cwd':str(work),'model':'gpt-6.1-terra','approvalPolicy':'never','sandbox':'read-only'})['thread']['id']
            client.thread_id=thread
            objective=pilot_fixture.CASES[case]['request']
            goal=client.call(3,'thread/goal/set',{'threadId':thread,'origin':'user','objective':objective,'status':'paused','tokenBudget':180000})['goal']
            report['goal_initial_status']=goal['status']
            goal=client.call(4,'thread/goal/get',{'threadId':thread})['goal']
            if goal['status'] != 'paused' or goal['objective'] != objective:
                raise worker.Failure('goal_not_persisted')
            if mode == 'protocol':
                report.update(status='protocol_verified',goal_status=goal['status'])
            else:
                goal=client.call(5,'thread/goal/set',{'threadId':thread,'origin':'user','status':'active'})['goal']
                report['goal_activated_status']=goal['status']
                client.generation_requested=True
                prompt=('Piloto con datos sintéticos. OpenAI Decisions seleccionó ingeniero web. '
                  'Lanza un subagente ingeniero web nativo: debe inspeccionar y registrar con record_plan al menos dos propuestas y errores previstos antes de probar, '
                  'crear un subagente ejecutor que implemente y pruebe una propuesta, esperar al ejecutor, '
                  'y validar personalmente el resultado. El padre inmediato recibe y comprueba el resultado. '
                  'Si no pasa, diagnostica, registra errores y consulta decide con opciones estancado o continuar. '
                  'Comunica al ejecutor el objetivo literal completo, incluidos los nombres exactos de campos. '
                  'Si su prueba falla, devuélvele el diagnóstico y espera a que él mismo pruebe la corrección; '
                  'el jefe debe probar después la misma receta. No declares completa la jerarquía si el ejecutor '
                  'no verificó la misma receta o su Goal quedó bloqueado. '
                  'Cambia de propuesta si está estancado; si no, reintenta con contexto. '
                  'Cada agente tiene un máximo de cuatro pruebas de receta; el jefe cuenta con su propio presupuesto. '
                  'No afirmes éxito sin prueba. Cumple el objetivo activo y reporta la jerarquía.')
                turn=client.call(6,'turn/start',{'threadId':thread,'cwd':str(work),'approvalPolicy':'never',
                    'model':'gpt-6.1-terra','effort':'medium','input':[{'type':'text','text':prompt}]})['turn']['id']
                while turn not in client.completed_turns:
                    client.next()
                goal=client.call(7,'thread/goal/get',{'threadId':thread})['goal']
                while goal['status']=='active' and client.root_turns_started < 4:
                    client.next()
                    goal=client.goal_updates.get(thread,goal)
                # El hijo puede terminar su turno y devolver evidencia sin cerrar su Goal.
                # Solo el supervisor cierra un Goal activo cuando hay pruebas coincidentes
                # del ejecutor y de su jefe inmediato y el turno hijo ya terminó.
                report['host_goal_closures']=[]
                for index,child in enumerate(client.executor_goals):
                    parent=client.agent_parents.get(child)
                    current=client.goal_updates.get(child,{})
                    if may_close_executor(child,parent,current,client.observations,client.recipe_evidence):
                        try:
                            closed=client.call(40+index,'thread/goal/set',{'threadId':child,'origin':'automatic','status':'complete'})['goal']
                            client.goal_updates[child]=closed
                            report['host_goal_closures'].append({'threadId':child,'status':closed['status'],'basis':'child_and_parent_verified'})
                        except worker.Failure as error:
                            report['host_goal_closures'].append({'threadId':child,'status':'failed','error':str(error)})
                report['goal_status']=goal['status']
                report['goal_details']=goal
                report['root_turns_started']=client.root_turns_started
                report['turn_status']=client.completed_turns[turn]
                report['status']='goal_terminal' if goal['status']!='active' else 'turn_budget_exhausted'
            report['thread_id']=thread
        except worker.Failure as error:
            report.update(status='protocol_or_runtime_failure',error=str(error))
        finally:
            report['codex_usage_by_thread']=client.thread_usage
            report['executor_goals']=client.executor_goals
            report['agent_parents']=client.agent_parents
            report['recipe_evidence']=client.recipe_evidence
            report['plan_evidence']=client.plan_evidence
            report['goal_updates_by_thread']=client.goal_updates
            report['observations']=client.observations
            report['messages']=list(client.messages.values())
            client.selector.close()
            if process.poll() is None:
                os.killpg(process.pid,signal.SIGTERM)
                try: process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid,signal.SIGKILL)
                    process.wait(timeout=5)
            report['session_metadata']=pilot_native_agents.collect_session_metadata(home)
            report['calls']=[json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
            report['process_exit_code']=process.returncode
        candidates=[x['arguments']['recipe'] for x in report['calls'] if x.get('tool')=='test_recipe' and isinstance(x.get('arguments'),dict) and 'recipe' in x['arguments']]
        report['verification']=pilot_fixture.verify(case,candidates[-1]) if candidates else None
    report['temporary_removed']=not root.exists()
    report['elapsed_ms']=round((time.monotonic()-start)*1000)
    report['source_drift']=any(common.sha_file(Path(worker.binary()) if key=='codex_binary' else ROOT/key)!=digest
                               for key,digest in fingerprints.items())
    if mode == 'live':
        report['architecture_audit']=audit(report)
    common.save(output/'result.json',report)
    return output,report


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case',required=True,choices=tuple(pilot_fixture.CASES))
    parser.add_argument('--mode',required=True,choices=('protocol','live'))
    parser.add_argument('--native-v1',action='store_true')
    parser.add_argument('--faults',type=int,choices=(0,1,2),default=0)
    args=parser.parse_args()
    path,result=run(args.case,args.mode,args.native_v1,args.faults)
    print(json.dumps({'directory':str(path),'status':result['status'],'goal_status':result['goal_status'],
                      'verification':result.get('verification'),'elapsed_ms':result.get('elapsed_ms')}))

#!/usr/bin/env python3
"""Agente Codex que construye un plugin real usando MCP de navegador aislado."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

import assignment_evaluation_worker as worker
import pilot_native_goal

ROOT=Path(__file__).resolve().parents[2]
OBJECTIVE='Crea y verifica un plugin de solo lectura id fixture, función extract sin parámetros, que obtenga todos los nombres del directorio en un array de cadenas.'


class Client(pilot_native_goal.Client):
    def __init__(self,process,until):
        super().__init__(process,until,'simple')
        self.plugin_evidence=[]

    def register_executor(self,child,path):
        if child in self.executor_goals:
            return
        identifier=100+len(self.executor_goals)
        self.executor_goals[child]={'request_id':identifier,'agent_path':path,'status':'requested'}
        self.send({'id':identifier,'method':'thread/goal/set','params':{'threadId':child,'origin':'user',
            'objective':OBJECTIVE+' Construye la función, verifícala y comunica evidencia a tu jefe.',
            'status':'active','tokenBudget':100000}})

    def observe(self,value):
        params=value.get('params',{})
        item=params.get('item',{})
        if value.get('method')=='item/completed' and isinstance(item,dict) and item.get('type')=='mcpToolCall' and item.get('tool')=='browser_plugin_run':
            args=item.get('arguments') or {}
            result=item.get('result') or {}
            data=result.get('structuredContent') if isinstance(result,dict) else None
            self.plugin_evidence.append({'threadId':params.get('threadId'),'action':args.get('action') if isinstance(args,dict) else None,
                'function':args.get('function_name') if isinstance(args,dict) else None,
                'status':data.get('status') if isinstance(data,dict) else None})
        super().observe(value)


def main():
    arm=os.environ['GENNIE_REAL_ARM']
    if arm not in ('direct','native'):
        raise ValueError('invalid_arm')
    output=Path(os.environ['GENNIE_REAL_AGENT_OUTPUT'])
    origin=os.environ['GENNIE_PILOT_ORIGIN']
    began=time.monotonic()
    report={'arm':arm,'status':'not_started'}
    with tempfile.TemporaryDirectory(prefix='gennie-real-agent-codex-') as temp:
        root=Path(temp)
        home,work=root/'home',root/'work'
        home.mkdir(mode=0o700);work.mkdir(mode=0o700)
        auth=Path(os.environ.get('CODEX_HOME',str(Path.home()/'.codex')))/'auth.json'
        if not auth.is_file():
            output.write_text(json.dumps({'status':'authentication_unavailable'}));return
        (home/'auth.json').symlink_to(auth)
        env=os.environ.copy();env['CODEX_HOME']=str(home)
        for name in ('OPENAI_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_PILOT_SETTINGS_FILE'):
            env.pop(name,None)
        configs=['mcp_servers.gennie.command='+json.dumps(os.environ['GENNIE_REAL_MCP_HELPER']),
            'mcp_servers.gennie.args='+json.dumps(['--mcp',os.environ['GENNIE_REAL_MCP_SOCKET']]),
            'mcp_servers.gennie.default_tools_approval_mode="approve"',
            'features.shell_tool=false','features.unified_exec=false','features.computer_use=false',
            'features.browser_use=false','features.browser_use_external=false','features.plugins=false',
            'features.apps=false','web_search="disabled"','model_reasoning_effort="medium"',
            'features.multi_agent='+('true' if arm=='native' else 'false')]
        if arm=='native':
            catalog=json.loads((Path.home()/'.codex/models_cache.json').read_text())
            selected=[x for x in catalog['models'] if x.get('slug')=='gpt-6.1-terra']
            if len(selected)!=1: raise RuntimeError('model_catalog_unavailable')
            selected[0]['multi_agent_version']='v1'
            path=root/'models.json';path.write_text(json.dumps({'models':catalog['models']}))
            trace=root/'decisions.jsonl'
            configs.extend(['model_catalog_json='+json.dumps(str(path)),'features.multi_agent_v2=false',
                'features.goals=true','agents.max_depth=2','agents.max_threads=3',
                'mcp_servers.decision.command='+json.dumps(sys.executable),
                'mcp_servers.decision.args='+json.dumps([str(ROOT/'scripts/probes/pilot_native_mcp.py')]),
                'mcp_servers.decision.env.GENNIE_PILOT_CASE="simple"',
                'mcp_servers.decision.env.GENNIE_PILOT_PLUGIN_MODE="1"',
                'mcp_servers.decision.env.GENNIE_PILOT_TRACE='+json.dumps(str(trace))])
        command=[worker.binary(),'app-server','--listen','stdio://']
        for item in configs: command.extend(['-c',item])
        process=subprocess.Popen(command,cwd=work,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,bufsize=0,start_new_session=True)
        client=Client(process,began+180)
        try:
            client.call(1,'initialize',{'clientInfo':{'name':'gennie_real_plugin_pilot','version':'1'},'capabilities':{'experimentalApi':True}})
            client.send({'method':'initialized','params':{}})
            thread=client.call(2,'thread/start',{'cwd':str(work),'model':'gpt-6.1-terra','approvalPolicy':'never','sandbox':'read-only'})['thread']['id']
            client.thread_id=thread
            if arm=='native':
                goal=client.call(3,'thread/goal/set',{'threadId':thread,'origin':'user','objective':OBJECTIVE,'status':'paused','tokenBudget':180000})['goal']
                client.call(4,'thread/goal/set',{'threadId':thread,'origin':'user','status':'active'})
            prompt=OBJECTIVE+f' El sitio autorizado es {origin}/main. La función debe usar transport=page y path=/main; guarda y verifica el plugin real. No uses otros destinos.'
            if arm=='native':
                prompt+=' Consulta decide para seleccionar entre web y desktop. Lanza un ingeniero web real; debe inspeccionar, registrar al menos dos propuestas con record_plan, delegar a un ejecutor real, esperar su prueba y comprobar personalmente la misma función. Ante errores diagnostica y consulta decide antes de reintentar. El ejecutor informa al jefe inmediato.'
            client.generation_requested=True
            turn=client.call(5,'turn/start',{'threadId':thread,'cwd':str(work),'approvalPolicy':'never','model':'gpt-6.1-terra','effort':'medium','input':[{'type':'text','text':prompt}]})['turn']['id']
            while turn not in client.completed_turns: client.next()
            report.update(status='turn_finished',turn_status=client.completed_turns[turn],thread_id=thread)
            if arm=='native':
                goal=client.call(6,'thread/goal/get',{'threadId':thread})['goal']
                while goal['status']=='active' and client.root_turns_started<4:
                    client.next()
                    goal=client.goal_updates.get(thread,goal)
                report['goal']=goal
                report['root_turns_started']=client.root_turns_started
                report['status']='goal_terminal' if goal['status']!='active' else 'turn_budget_exhausted'
        except worker.Failure as error:
            report.update(status='runtime_failure',error=str(error))
        finally:
            report['usage_by_thread']=client.thread_usage
            report['goal_updates_by_thread']=client.goal_updates
            report['executor_goals']=client.executor_goals
            report['agent_parents']=client.agent_parents
            report['plan_evidence']=client.plan_evidence
            report['plugin_evidence']=client.plugin_evidence
            report['messages']=list(client.messages.values())
            if arm=='native': report['decisions']=[json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
            client.selector.close()
            if process.poll() is None:
                os.killpg(process.pid,signal.SIGTERM)
                try:process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid,signal.SIGKILL);process.wait(timeout=5)
    report['elapsed_ms']=round((time.monotonic()-began)*1000)
    output.write_text(json.dumps(report,ensure_ascii=False))


if __name__=='__main__':
    main()

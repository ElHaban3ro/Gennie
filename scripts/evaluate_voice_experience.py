#!/usr/bin/env python3
"""Evalúa lenguaje y contexto con Laya real, inventario simulado y ningún efecto."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import time


def fixture_context():
    apps = [('com.hnc.Discord','Discord'),('com.apple.Safari','Safari'),
            ('com.apple.finder','Finder'),('com.microsoft.VSCode','Visual Studio Code'),
            ('com.adobe.PremierePro.26','Adobe Premiere Pro 2026'),
            ('com.spotify.client','Spotify'),('com.apple.Music','Music')]
    applications = {bundle: {'name':name,'path':'/fixture/'+name+'.app',
                            'aliases':[name], 'short_aliases':[name]} for bundle,name in apps}
    applications['com.apple.Music']['aliases'] += ['Música','Música de Apple','Apple Music']
    applications['com.microsoft.VSCode']['aliases'] += ['VS Code','Visual Studio Code']
    return {'applications':applications, 'installed':{bundle:True for bundle,_ in apps},
            'running':['com.spotify.client'], 'frontmost':'com.apple.finder',
            'browser':'com.apple.Safari', '_players_loaded':True,
            'players':{'com.spotify.client':{'state':'playing','track':'fixture-track'}}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model',type=Path,required=True)
    parser.add_argument('--cases',type=Path,default=Path(__file__).parent/'fixtures/voice_experience_cases.json')
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--worker-source',type=Path)
    parser.add_argument('--split',choices=['development','holdout','all'],default='all')
    parser.add_argument('--trace',action='store_true')
    args = parser.parse_args()
    os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',USE_TF='0',TOKENIZERS_PARALLELISM='false')
    python = Path(__file__).resolve().parents[1]/'apps/decisions/python'
    source = args.worker_source.read_text() if args.worker_source else '\n'.join((python/f).read_text() for f in ('applications.py','domain.py','worker.py'))
    worker = {'__name__':'experience_worker','__file__':str(python/'worker.py')}
    exec(compile(source,str(args.worker_source or python/'worker.py'),'exec'),worker)
    started = time.perf_counter()
    agent = worker['load_local_agent'](args.model)
    load_seconds = time.perf_counter()-started
    original = worker['choice']
    trace = []
    def traced(*values):
        answer = original(*values)
        if args.trace:
            trace.append({'question':values[2],'state':values[1],'choice':answer[0],'probability':answer[1]})
        return answer
    worker['choice'] = traced
    cases = json.loads(args.cases.read_text())['cases']
    selected = [case for case in cases if args.split=='all' or case['split']==args.split]
    results = []
    for case in selected:
        trace.clear()
        started = time.perf_counter()
        try:
            prediction = worker['decide'](agent,case['request'],fixture_context())
            executable = prediction['route']=='complete' and bool(prediction.get('target')) and prediction.get('availability',{}).get('status')=='available'
            expected = case['expected']
            correct = executable==expected['executable'] and all(prediction.get(key)==value for key,value in expected.items() if key!='executable')
            row = {**case,'prediction':prediction,'passed':correct,'false_execution':executable and not expected['executable'],
                   'wrong_action':executable and expected['executable'] and not correct}
        except Exception as error:
            row = {**case,'error':type(error).__name__,'passed':False,'false_execution':False,'wrong_action':False}
        row['latency_ms'] = round((time.perf_counter()-started)*1000,2)
        if args.trace:
            row['trace'] = list(trace)
        results.append(row)
        if len(results)%20==0:
            print(json.dumps({'evaluated':len(results),'total':len(selected)}),flush=True)
    groups = {}
    for group in sorted({row['group'] for row in results}):
        subset = [row for row in results if row['group']==group]
        groups[group] = {'count':len(subset),'passed':sum(row['passed'] for row in subset),
                         'false_executions':sum(row['false_execution'] for row in subset)}
    report = {'execution':'none','context':'simulated_apps_and_music; explicitly supplied conversational context',
              'device':str(agent.device),'dtype':str(agent.dtype),
              'split':args.split,'source_sha256':hashlib.sha256(source.encode()).hexdigest(),
              'corpus_sha256':hashlib.sha256(args.cases.read_bytes()).hexdigest(),
              'model_load_seconds':load_seconds,'count':len(results),
              'passed':sum(row['passed'] for row in results),
              'false_executions':sum(row['false_execution'] for row in results),
              'wrong_actions':sum(row['wrong_action'] for row in results),
              'errors':sum('error' in row for row in results),
              'median_latency_ms':statistics.median(row['latency_ms'] for row in results) if results else None,
              'groups':groups,'results':results}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({key:value for key,value in report.items() if key not in ('groups','results')}))
    return 0 if report['passed']==report['count'] else 1


if __name__=='__main__':
    raise SystemExit(main())

#!/usr/bin/env python3
"""Regresión breve con Laya real y contexto simulado; no ejecuta acciones."""
import argparse
import copy
import importlib.util
import json
import os
from pathlib import Path
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--model',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--exact-only',action='store_true')
args = parser.parse_args()
os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',USE_TF='0',TOKENIZERS_PARALLELISM='false')
spec = importlib.util.spec_from_file_location('worker',Path(__file__).resolve().parents[1]/'apps/decisions/python/worker.py')
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
started = time.monotonic()
agent = worker.load_local_agent(args.model)
context = {
    'installed':{bundle:True for bundle in worker.BUNDLES.values()},
    'running':['com.spotify.client'],'browser':'com.apple.Safari',
    'players':{'com.spotify.client':{'state':'playing','track':'fixture'}},
    'applications':{bundle:{'name':name,'aliases':aliases,'path':'/fixture/'+name+'.app'} for bundle,name,aliases in [
        ('com.apple.Music','Music',['Music','Música']),('com.spotify.client','Spotify',['Spotify']),
        ('com.hnc.Discord','Discord',['Discord']),('com.apple.Safari','Safari',['Safari']),
    ]},
}
cases = [
    ({'text':'Pausa la música.'},'pause_music','com.spotify.client'),
    ({'text':'Pausa la música.','last_action':'open_app','last_target':'com.apple.Music'},'pause_music','com.spotify.client'),
    ({'text':'Reanuda la música.'},'play_music','com.spotify.client'),
    ({'text':'Siguiente canción.'},'next_track','com.spotify.client'),
    ({'text':'Canción anterior.'},'previous_track','com.spotify.client'),
    ({'text':'Pausa Spotify.'},'pause_music','com.spotify.client'),
    ({'text':'No pauses la música.'},None,None),
    ({'text':'Nunca pauses Spotify.'},None,None),
    ({'text':'Ayer pausé la música.'},None,None),
    ({'text':'¿Qué significa pausar la música?'},None,None),
    ({'text':'Abre Música.'},'open_app','com.apple.Music'),
    ({'text':'Abre Discord.'},'open_app','com.hnc.Discord'),
    ({'text':'No abras Música.'},None,None),
]
if args.exact_only:
    cases = cases[:1]
results = []
for request,action,target in cases:
    result = worker.decide(agent,request,copy.deepcopy(context))
    actual = (result['action'],result['target']) if result['route']=='complete' else (None,None)
    results.append({'request':request,'expected_action':action,'expected_target':target,
                    'prediction':result,'passed':actual==(action,target)})
report = {'execution':'none','context':'simulated','count':len(results),
          'passed':sum(row['passed'] for row in results),'elapsed_seconds':round(time.monotonic()-started,2),'results':results}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False),flush=True)
raise SystemExit(0 if report['passed']==report['count'] else 1)

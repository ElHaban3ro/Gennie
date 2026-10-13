#!/usr/bin/env python3
"""Regresión con Laya real: órdenes claras y frases que nunca deben causar efectos."""
import argparse
import importlib.util
import json
import os
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--model',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args = parser.parse_args()
os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',USE_TF='0',TOKENIZERS_PARALLELISM='false')
spec = importlib.util.spec_from_file_location('worker',Path(__file__).resolve().parents[1]/'apps/decisions/python/worker.py')
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
import laya
agent = laya.load(str(args.model),device='mps')
context = {'installed':{bundle:True for bundle in worker.BUNDLES.values()},'running':[],
           'browser':'com.apple.Safari','players':{}}
cases = [('Abre Safari.','open_safari'),('Abre Finder.','open_files'),
         ('No abras Safari.',None),('solo dime si entendiste: abre Discord',None),
         ('ayer abrí Safari',None),('abre el',None),('busca gatos en Safari',None)]
results = []
for text,action in cases:
    prediction = worker.decide(agent,{'text':text},context)
    actual = prediction['action'] if prediction['route']=='complete' else None
    results.append({'text':text,'expected_action':action,'prediction':prediction,'passed':actual==action})
report = {'execution':'none','context':'simulated','count':len(results),'passed':sum(r['passed'] for r in results),'results':results}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
raise SystemExit(0 if report['passed']==report['count'] else 1)

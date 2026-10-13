#!/usr/bin/env python3
"""Comprueba entidades y decisiones con Laya real y apps instaladas; sin efectos."""
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
agent = worker.load_local_agent(args.model)
context = worker.system_context()
cases = [('Abre Discord.','com.hnc.Discord'),('Abre Discord','com.hnc.Discord'),('abre Discord.','com.hnc.Discord'),
         ('Por favor, abre Discord.','com.hnc.Discord'),('Abre Safari.','com.apple.Safari'),('Abre Finder.','com.apple.finder'),('Abre Premiere.','com.adobe.PremierePro.26'),
         ('Por favor abre Visual Studio Code.','com.microsoft.VSCode'),
         ('No abras Discord.',None),('no abras Discord',None),('No quiero que abras Discord.',None),
         ('Nunca abras Discord.',None),('solo dime si entendiste: abre Discord',None),
         ('La frase dice abre Discord.',None),('Ayer abrí Discord.',None),
         ('busca gatos en Discord',None),('Abre Discord y Safari.',None),
         ('abre una aplicación que no existe FerFixture',None),
         ('No abras Discord por favor.',None),('Por favor, no abras Discord.',None),
         ('¿Puedes decirme qué significa abre Discord?',None),('Si digo abre Discord, ¿qué harías?',None),
         ('No abras Safari.',None),('Nunca abras Safari.',None),('No quiero abrir Finder.',None),
         ('Quiero que busques un mensaje en Discord.',None),('Abre',None)]
results = []
for text,target in cases:
    result = worker.decide(agent,{'text':text},context)
    actual = result['target'] if result['route']=='complete' else None
    results.append({'text':text,'expected_target':target,'prediction':result,'passed':actual==target})
for request,target in [({'text':'Abre Discord.','pending':'Abre Discord.','pending_action':'open_app'},'com.hnc.Discord'),
                       ({'text':'Abre Discord.','last_action':'open_app','last_target':'com.apple.Safari'},'com.hnc.Discord'),
                       ({'text':'No abras Discord.','pending':'Abre Discord.','pending_action':'open_app'},None),
                       ({'text':'Nunca abras Discord.','pending':'Nunca abras Discord.','pending_action':'open_app'},None)]:
    result = worker.decide(agent,request,context)
    actual = result['target'] if result['route']=='complete' else None
    results.append({'request':request,'expected_target':target,'prediction':result,'passed':actual==target})
report = {'execution':'none','context':'real_system_snapshot','applications':len(context['applications']),
          'count':len(results),'passed':sum(row['passed'] for row in results),'results':results}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
raise SystemExit(0 if report['passed']==report['count'] else 1)

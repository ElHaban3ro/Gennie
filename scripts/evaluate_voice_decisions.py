#!/usr/bin/env python3
"""Entrega transcripciones sintéticas reales a Laya en preview; nunca abre micrófono."""
import argparse
import importlib.util
import json
import os
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--adapter', type=Path)
parser.add_argument('--fixture', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
fixture = json.loads(args.fixture.read_text())
if fixture.get('audio_source') != 'synthetic' or fixture.get('microphone') is not False:
    parser.error('Solo admite fixtures de audio sintético')
os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',USE_TF='0',TOKENIZERS_PARALLELISM='false')
spec = importlib.util.spec_from_file_location('worker',Path(__file__).resolve().parents[1]/'apps/decisions/python/worker.py')
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
agent = worker.load_local_agent(args.model)
if args.adapter:
    agent.domain_adapter = worker.DomainAdapter(agent,args.adapter,worker.file_hash(args.model/'model.safetensors'))
from evaluate_voice_experience import fixture_context
context = fixture_context()
records = []
for record in fixture['records']:
    result = dict(record)
    text = record['transcription']
    prediction = worker.decide(agent,{'text':text},context) if text.strip() else None
    result['prediction'] = prediction
    actual = (prediction['action'] if prediction['route']=='complete' else prediction['route']) if prediction else 'no_speech'
    if actual=='open_app':
        actual = next((action for action,bundle in worker.BUNDLES.items() if bundle==prediction['target']),'open_app')
    result['actual'] = actual
    result['correct'] = actual==record['expected']
    executable = bool(prediction and prediction['route']=='complete' and prediction.get('target')
                      and (prediction.get('availability') or {}).get('status')=='available')
    result['false_execution'] = record['expected'] not in worker.CATALOG and executable
    records.append(result)
report = {'audio_source':'synthetic','microphone':False,'execution':'none','context':'simulated',
          'count':len(records),'transcribed':sum(bool(r['transcription'].strip()) for r in records),
          'correct':sum(r['correct'] for r in records),'false_executions':sum(r['false_execution'] for r in records),'records':records}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k!='records'}))

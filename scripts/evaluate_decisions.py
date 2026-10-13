#!/usr/bin/env python3
"""Mide decisiones reales de Laya con contexto simulado; no ejecuta acciones."""
import argparse
from collections import Counter
import importlib.util
import json
import os
from pathlib import Path
import random
import time

parser = argparse.ArgumentParser()
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--library', type=Path, required=True)
parser.add_argument('--limit', type=int, default=120)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--dataset', choices=['controlled', 'challenge'])
parser.add_argument('--adapter', type=Path)
args = parser.parse_args()
os.environ.update(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1', HF_HUB_DISABLE_TELEMETRY='1', USE_TF='0', TOKENIZERS_PARALLELISM='false')
spec = importlib.util.spec_from_file_location('worker', Path(__file__).resolve().parents[1]/'apps/decisions/python/worker.py')
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
import laya
agent = laya.load(str(args.model.resolve()), device='mps')
if args.adapter:
    agent.domain_adapter = worker.DomainAdapter(agent, args.adapter, worker.file_hash(args.model/'model.safetensors'))
rows = [json.loads(line) for line in args.library.read_text().splitlines()]
if args.dataset:
    dataset = 'independent' if args.dataset == 'challenge' else args.dataset
    rows = [row for row in rows if row['dataset'] == dataset]
if not rows:
    parser.error('No hay frases para el dataset indicado')
groups = {}
for row in rows:
    groups.setdefault(row['expected_action'], []).append(row)
rng = random.Random(20261012)
selected = []
for group in groups.values():
    rng.shuffle(group)
while len(selected) < min(args.limit, len(rows)):
    for group in groups.values():
        if group and len(selected) < args.limit:
            selected.append(group.pop())
results = []
for row in selected:
    ctx = {'installed': {bundle: True for bundle in worker.BUNDLES.values()}, 'running': ['com.spotify.client'],
           'browser': 'com.apple.Safari', 'players': {'com.spotify.client': {'state': 'playing' if row['state']['context'].get('music_playing') else 'paused', 'track': 'fixture'}}}
    request = {'text': row['state']['utterance'], 'last_action': row['state']['context'].get('last_action')}
    if row['state']['context'].get('pending_app'):
        request['pending'] = 'Abrir ' + row['state']['context']['pending_app']
    started = time.perf_counter()
    result = worker.decide(agent, request, ctx)
    actual = result['action'] if result['route'] == 'complete' else 'god_call'
    results.append({'id': row['id'], 'text': request['text'], 'expected': row['expected_action'],
                    'prediction': result, 'correct': actual == row['expected_action'],
                    'false_execution': row['expected_action'] == 'god_call' and actual != 'god_call',
                    'milliseconds': (time.perf_counter()-started)*1000})
    if len(results) % 20 == 0:
        print(f'{len(results)}/{len(selected)}', flush=True)
summary = {'count': len(results), 'correct': sum(r['correct'] for r in results),
           'false_executions': sum(r['false_execution'] for r in results),
           'routes': dict(Counter(r['prediction']['route'] for r in results)),
           'context': 'simulated', 'execution': 'none', 'adapter': str(args.adapter) if args.adapter else None, 'results': results}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(summary, ensure_ascii=False, indent=2)+'\n')
print(json.dumps({k: v for k, v in summary.items() if k != 'results'}), flush=True)

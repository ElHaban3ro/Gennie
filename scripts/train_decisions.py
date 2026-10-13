#!/usr/bin/env python3
"""Entrena una cabeza sobre Laya congelado; separa familias léxicas sin ejecutar acciones."""
import argparse
from collections import defaultdict
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import random
import re
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--library', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--epochs', type=int, default=100)
parser.add_argument('--finetune', action='store_true')
parser.add_argument('--typed', action='store_true')
args = parser.parse_args()
os.environ.update(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1', USE_TF='0', TOKENIZERS_PARALLELISM='false')
root = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('domain', root/'apps/decisions/python/domain.py')
domain = importlib.util.module_from_spec(spec)
spec.loader.exec_module(domain)
import torch
from safetensors.torch import save_file
torch.set_num_threads(2)
torch.manual_seed(20261012)
rng = random.Random(20261012)
args.output.mkdir(parents=True, exist_ok=True)
rows = [json.loads(line) for line in args.library.read_text().splitlines()]
samples = []


def lexical_group(text):
    text = text.lower().strip(' .,!¿?')
    prefix = r'^(?:oye|mira|eh|bueno|a ver|porfa|por favor|ahora sí)\b[, ]*'
    suffix = r'[, ]*\b(?:por favor|porfa|si puedes|cuando puedas|un momento)$'
    for _ in range(12):
        before = text
        text = re.sub(prefix, '', text)
        text = re.sub(suffix, '', text).strip()
        if text == before:
            break
    return text


for row in rows:
    if row['dataset'] != 'controlled':
        continue
    label = row['expected_action']
    if label == 'god_call' and row['family'] == 'god_call_negation':
        label = 'no_order'
    elif label == 'god_call' and row['family'] == 'missing_context':
        label = 'ambiguous'
    request = {'text': row['state']['utterance'], 'last_action': row['state']['context'].get('last_action')}
    pending_app = row['state']['context'].get('pending_app')
    if pending_app:
        request['pending'] = 'Abrir ' + pending_app
    samples.append({'id': row['id'], 'label': label, 'request': request, 'group': lexical_group(request['text'])})

# Anotaciones explícitas del flujo nuevo: el corpus original agrupa varias rutas como god_call.
extras = {
 'incomplete': ['abre', 'abre el', 'quiero abrir', 'necesito que abras', 'por favor abre la aplicación', 'me puedes abrir', 'quiero que pongas', 'vamos a abrir', 'ponme el', 'inicia la', 'saca el', 'me lanzas', 'puedes poner', 'necesito abrir', 'hazme el favor de abrir'],
 'ambiguous': ['hazlo', 'ábrelo', 'abre eso', 'pon esa aplicación', 'otra vez', 'repítelo', 'haz lo mismo otra vez', 'esa no, la otra', 'abre la aplicación que dije', 'lo de antes', 'cambia eso', 'pásalo', 'abre música', 'abre lo que hablamos', 'pon la otra'],
 'no_order': ['me gusta Safari', 'estoy escribiendo la frase abre Chrome', 'la frase dice pausa la música', 'ayer abrí Spotify', 'no abras Chrome', 'no quiero que pauses la música', 'déjalo como está', 'no hagas nada', 'olvida la orden anterior', 'estoy contando que pasé una canción', 'hola', 'buenas tardes', 'gracias', 'Safari funciona bien', 'esa canción es bonita'],
}
for label, texts in extras.items():
    for index, text in enumerate(texts):
        for filler in ['', 'oye, ', 'por favor, ', 'a ver, ']:
            samples.append({'id': f'route-{label}-{index}-{len(filler)}', 'label': label, 'request': {'text': filler+text}, 'group': lexical_group(text)})
for action in domain.DOMAIN_ACTIONS:
    for phrase in ['otra vez', 'repítelo', 'haz lo mismo otra vez', 'lo mismo nuevamente', 'repite lo anterior', 'vuelve a hacerlo']:
        samples.append({'id': 'context:'+action+phrase, 'label': action, 'request': {'text': phrase, 'last_action': action}, 'group': lexical_group(phrase)})
    if action:
        for phrase in ['ábrelo', 'esa aplicación', 'sí, ábrela', 'abre esa', 'quiero abrirla']:
            samples.append({'id': 'pending:'+action+phrase, 'label': action, 'request': {'text': phrase, 'pending': domain.DESCRIPTIONS[domain.DOMAIN_ACTIONS.index(action)], 'pending_action': action}, 'group': lexical_group(phrase)})
        if not action.startswith('open_'):
            for phrase in ['Spotify', 'Música de Apple', 'en Spotify', 'Apple Music', 'el de Apple']:
                samples.append({'id': 'player:'+action+phrase, 'label': action, 'request': {'text': phrase, 'pending': domain.DESCRIPTIONS[domain.DOMAIN_ACTIONS.index(action)], 'pending_action': action}, 'group': lexical_group(phrase)})

groups = defaultdict(list)
for index, sample in enumerate(samples):
    groups[sample['group']].append(index)
ordered = sorted(groups)
rng.shuffle(ordered)
split_by_group = {group: ('train' if i % 10 < 7 else 'calibration' if i % 10 < 9 else 'test') for i, group in enumerate(ordered)}
indices = {name: [i for i, sample in enumerate(samples) if split_by_group[sample['group']] == name] for name in ('train', 'calibration', 'test')}
assert set(indices['train']).isdisjoint(indices['calibration']) and set(indices['train']).isdisjoint(indices['test'])
assert set(sample['label'] for sample in (samples[i] for i in indices['train'])) == set(domain.DOMAIN_LABELS), 'Falta clase en entrenamiento'
split_file = {'seed': 20261012, 'groups': split_by_group, 'samples': [{'id': s['id'], 'label': s['label'], 'group': s['group'], 'split': split_by_group[s['group']]} for s in samples]}
(args.output/'split.json').write_text(json.dumps(split_file, ensure_ascii=False, indent=2)+'\n')
if args.finetune:
    exec((root/('scripts/train_decisions_typed.py' if args.typed else 'scripts/train_decisions_encoder.py')).read_text())
    raise SystemExit(0)
base_hash = domain.file_hash(args.model/'model.safetensors')
cache = args.output/'features.safetensors'
identity = hashlib.sha256(json.dumps(samples, ensure_ascii=False, sort_keys=True).encode()).hexdigest()
if cache.exists() and (args.output/'features.json').exists() and json.loads((args.output/'features.json').read_text()) == {'corpus': identity, 'base': base_hash}:
    from safetensors.torch import load_file
    features = load_file(str(cache))['features']
else:
    import laya
    print('Cargando encoder congelado de Laya', flush=True)
    agent = laya.load(str(args.model), device='mps')
    features = []
    for start in range(0, len(samples), 16):
        features.append(domain.encode(agent, [s['request'] for s in samples[start:start+16]]))
        if start % 256 == 0:
            print(f'Encoder {start}/{len(samples)}', flush=True)
    features = torch.cat(features)
    save_file({'features': features.contiguous()}, str(cache))
    (args.output/'features.json').write_text(json.dumps({'corpus': identity, 'base': base_hash}))
    del agent
    torch.mps.empty_cache()

y = torch.tensor([domain.DOMAIN_LABELS.index(s['label']) for s in samples])
mean = features[indices['train']].mean(0)
scale = features[indices['train']].std(0).clamp_min(.05)
x = (features - mean) / scale
head = domain.make_head(x.shape[1], len(domain.DOMAIN_LABELS))
optimizer = torch.optim.AdamW(head.parameters(), lr=.001, weight_decay=.01)
counts = torch.bincount(y[indices['train']], minlength=len(domain.DOMAIN_LABELS)).float().clamp_min(1)
weights = (counts.sum()/counts).sqrt()
weights /= weights.mean()
loss_fn = torch.nn.CrossEntropyLoss(weight=weights)
best = None
best_loss = float('inf')
for epoch in range(args.epochs):
    head.train()
    permutation = torch.tensor(indices['train'])[torch.randperm(len(indices['train']))]
    for batch in permutation.split(128):
        optimizer.zero_grad()
        loss = loss_fn(head(x[batch]), y[batch])
        loss.backward()
        optimizer.step()
    head.eval()
    with torch.no_grad():
        val_loss = float(loss_fn(head(x[indices['calibration']]), y[indices['calibration']]))
    if val_loss < best_loss:
        best_loss, best = val_loss, {k: v.clone() for k, v in head.state_dict().items()}
    if epoch % 10 == 0:
        print(f'Cabeza época {epoch}; loss calibración {val_loss:.4f}', flush=True)
head.load_state_dict(best)
head.eval()
with torch.no_grad():
    probabilities = head(x).softmax(-1)
values, predicted = probabilities.max(-1)
calibration = torch.tensor(indices['calibration'])
local = predicted[calibration] < len(domain.DOMAIN_ACTIONS)
candidates = sorted(set([0.0, 1.0] + values[calibration][local].tolist()))
threshold = 1.0
accepted_best = 0
for candidate in candidates:
    accepted = local & (values[calibration] >= candidate)
    n = int(accepted.sum())
    wrong = int(((predicted[calibration] != y[calibration]) & accepted).sum())
    if wrong == 0 and n > accepted_best:
        threshold, accepted_best = candidate, n


def metrics(name):
    ix = torch.tensor(indices[name])
    accepted = (predicted[ix] < len(domain.DOMAIN_ACTIONS)) & (values[ix] >= threshold)
    wrong = (predicted[ix] != y[ix]) & accepted
    return {'count': len(ix), 'raw_correct': int((predicted[ix] == y[ix]).sum()), 'accepted_actions': int(accepted.sum()), 'wrong_accepted_actions': int(wrong.sum()), 'false_executions': int((wrong & (y[ix] >= len(domain.DOMAIN_ACTIONS))).sum())}
state = dict(head.state_dict(), feature_mean=mean, feature_scale=scale)
save_file({k:v.contiguous() for k,v in state.items()}, str(args.output/'head.safetensors'))
manifest = {'version': 1, 'labels': domain.DOMAIN_LABELS, 'dimension': x.shape[1], 'base_sha256': base_hash, 'head_sha256': domain.file_hash(args.output/'head.safetensors'), 'corpus_sha256': domain.file_hash(args.library), 'threshold': threshold, 'splits': {name: metrics(name) for name in indices}, 'trained_encoder': False, 'execution': 'none'}
(args.output/'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n')
print(json.dumps(manifest), flush=True)

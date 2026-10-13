"""Etapa de entrenamiento invocada por train_decisions.py, con su partición ya validada."""
import laya
from safetensors.torch import save_file
print('Adaptación de la última capa del encoder de Laya', flush=True)
agent = laya.load(str(args.model), device='mps')
encoder = agent.model.encoder
for parameter in encoder.parameters():
    parameter.requires_grad_(False)
for parameter in encoder.layers[-1].parameters():
    parameter.requires_grad_(True)
head = domain.make_head(encoder.config.hidden_size * 2, len(domain.DOMAIN_LABELS)).to(agent.device)
parameters = list(encoder.layers[-1].parameters())
optimizer = torch.optim.AdamW([{'params': parameters, 'lr': .00002}, {'params': head.parameters(), 'lr': .001}], weight_decay=.01)
y = torch.tensor([domain.DOMAIN_LABELS.index(sample['label']) for sample in samples])
counts = torch.bincount(y[indices['train']], minlength=len(domain.DOMAIN_LABELS)).float().clamp_min(1)
weights = (counts.sum()/counts).sqrt()
weights /= weights.mean()
loss_fn = torch.nn.CrossEntropyLoss(weight=weights.to(agent.device))


def features_for(batch):
    tokens = agent.tok([domain.semantic_text(samples[int(i)]['request']) for i in batch], padding=True, return_tensors='pt')
    if tokens['input_ids'].shape[1] > 512:
        raise ValueError('Texto demasiado largo')
    tokens = {k: v.to(agent.device) for k, v in tokens.items() if k in ('input_ids', 'attention_mask')}
    hidden = encoder(**tokens).last_hidden_state.float()
    mask = tokens['attention_mask'].unsqueeze(-1)
    return torch.cat([hidden[:, 0], (hidden * mask).sum(1)/mask.sum(1).clamp_min(1)], 1)


def evaluate(ix):
    encoder.eval()
    head.eval()
    output = []
    with torch.no_grad():
        for batch in torch.tensor(ix).split(16):
            output.append(head(features_for(batch)).cpu())
    return torch.cat(output)


best_loss = float('inf')
best = None
for epoch in range(min(args.epochs, 8)):
    encoder.eval()  # No actualizar capas congeladas ni activar su dropout.
    head.train()
    permutation = torch.tensor(indices['train'])[torch.randperm(len(indices['train']))]
    for step, batch in enumerate(permutation.split(16)):
        optimizer.zero_grad()
        logits = head(features_for(batch))
        loss = loss_fn(logits, y[batch].to(agent.device))
        loss.backward()
        torch.nn.utils.clip_grad_norm_(parameters + list(head.parameters()), 1)
        optimizer.step()
        if step % 64 == 0:
            print(f'Encoder época {epoch}, batch {step}', flush=True)
    val = evaluate(indices['calibration'])
    score = float(torch.nn.functional.cross_entropy(val, y[indices['calibration']]))
    print(f'Encoder calibración loss={score:.4f}', flush=True)
    if score < best_loss:
        best_loss = score
        best = {'head': {k: v.detach().cpu().clone() for k,v in head.state_dict().items()}, 'encoder': {k:v.detach().cpu().clone() for k,v in encoder.layers[-1].state_dict().items()}}
    elif epoch >= 2:
        break
head.load_state_dict(best['head'])
encoder.layers[-1].load_state_dict(best['encoder'])
cal = evaluate(indices['calibration']).softmax(-1)
values, prediction = cal.max(-1)
truth = y[indices['calibration']]
threshold, accepted_best = 1.0, 0
for candidate in sorted(set([0., 1.] + values.tolist())):
    accepted = (prediction < len(domain.DOMAIN_ACTIONS)) & (values >= candidate)
    wrong = ((prediction != truth) & accepted).sum().item()
    n = accepted.sum().item()
    if n >= 20 and wrong/n <= .01 and n > accepted_best:
        threshold, accepted_best = candidate, n


def metrics(ix, probabilities=None):
    probabilities = evaluate(ix).softmax(-1) if probabilities is None else probabilities
    score, predicted = probabilities.max(-1)
    truth = y[ix]
    accepted = (predicted < len(domain.DOMAIN_ACTIONS)) & (score >= threshold)
    wrong = accepted & (predicted != truth)
    return {'count': len(ix), 'raw_correct': int((predicted == truth).sum()), 'accepted_actions': int(accepted.sum()), 'wrong_accepted_actions': int(wrong.sum()), 'false_executions': int((wrong & (truth >= len(domain.DOMAIN_ACTIONS))).sum())}
state = dict(best['head'], feature_mean=torch.zeros(encoder.config.hidden_size*2), feature_scale=torch.ones(encoder.config.hidden_size*2))
save_file(state, str(args.output/'head.safetensors'))
save_file(best['encoder'], str(args.output/'encoder-last.safetensors'))
manifest = {'version': 1, 'labels': domain.DOMAIN_LABELS, 'dimension': encoder.config.hidden_size*2, 'base_sha256': domain.file_hash(args.model/'model.safetensors'), 'head_sha256': domain.file_hash(args.output/'head.safetensors'), 'encoder_sha256': domain.file_hash(args.output/'encoder-last.safetensors'), 'corpus_sha256': domain.file_hash(args.library), 'threshold': threshold, 'splits': {'calibration': metrics(indices['calibration'], cal), 'test': metrics(indices['test'])}, 'trained_encoder': True, 'execution': 'none'}
(args.output/'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n')
print(json.dumps(manifest), flush=True)

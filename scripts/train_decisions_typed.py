"""Ajusta la pregunta tipada y el scorer originales de Laya; no un clasificador de CLS."""
import laya
from laya.common import build_sequence, collate_items
from safetensors.torch import save_file
print('Adaptación de preguntas tipadas de Laya', flush=True)
agent = laya.load(str(args.model), device='mps')
model = agent.model
for parameter in model.parameters():
    parameter.requires_grad_(False)
for component in [model.encoder.layers[-1], model.head, model.scorer, model.type_emb]:
    for parameter in component.parameters():
        parameter.requires_grad_(True)
head_parameters = [p for name,p in model.named_parameters() if p.requires_grad and not name.startswith('encoder.')]
encoder_parameters = [p for p in model.encoder.layers[-1].parameters()]
optimizer = torch.optim.AdamW([{'params': encoder_parameters, 'lr': .000005}, {'params': head_parameters, 'lr': .00003}], weight_decay=.01)
question = {'t': 'choice', 'ins': 'Clasifica la intención final del usuario. Resuelve correcciones y referencias con contexto. Negar o citar una acción no ordena ejecutarla.',
            'crit': {chr(65+i): value for i,value in enumerate(domain.DESCRIPTIONS + ['Orden incompleta: faltan palabras.', 'Orden ambigua: falta referente.', 'No hay orden: negada, citada o conversación.', 'Orden fuera del catálogo local.'])}}
items = []
for sample in samples:
    ids, markers = build_sequence(agent.tok, domain.semantic_text(sample['request']), question, 1024, 256)
    assert len(markers) == len(domain.DOMAIN_LABELS)
    items.append([{'ids': ids, 'markers': markers, 'qtype': 0}])
y = torch.tensor([domain.DOMAIN_LABELS.index(sample['label']) for sample in samples])


def forward(batch):
    data = collate_items([items[int(i)] for i in batch], agent.tok.pad_token_id)
    logits, _ = model(*(data[key].to(agent.device) for key in ('input_ids', 'attention_mask', 'marker_pos', 'marker_mask', 'qtype')))
    return logits


def evaluate(ix):
    model.eval()
    output = []
    with torch.no_grad():
        for batch in torch.tensor(ix).split(8):
            output.append(forward(batch).cpu())
    return torch.cat(output)


best_loss, best = float('inf'), None
for epoch in range(min(args.epochs, 5)):
    model.eval()
    model.head.train()
    permutation = torch.tensor(indices['train'])[torch.randperm(len(indices['train']))]
    for step,batch in enumerate(permutation.split(8)):
        optimizer.zero_grad()
        loss = torch.nn.functional.cross_entropy(forward(batch), y[batch].to(agent.device))
        loss.backward()
        torch.nn.utils.clip_grad_norm_(head_parameters + encoder_parameters, 1)
        optimizer.step()
        if step % 64 == 0:
            print(f'Pregunta época {epoch}, batch {step}', flush=True)
    val = evaluate(indices['calibration'])
    score = float(torch.nn.functional.cross_entropy(val, y[indices['calibration']]))
    print(f'Pregunta calibración loss={score:.4f}', flush=True)
    if score < best_loss:
        best_loss = score
        names = {name for name,p in model.named_parameters() if p.requires_grad}
        best = {key: value.detach().cpu().clone() for key,value in model.state_dict().items() if key in names}
    elif epoch >= 1:
        break
model.load_state_dict(best, strict=False)
cal = evaluate(indices['calibration']).softmax(-1)
values, predicted = cal.max(-1)
truth = y[indices['calibration']]
threshold, accepted_best = 1., 0
for candidate in sorted(set([0., 1.] + values.tolist())):
    accepted = (predicted < len(domain.DOMAIN_ACTIONS)) & (values >= candidate)
    n = int(accepted.sum())
    wrong = int(((predicted != truth) & accepted).sum())
    if n >= 20 and wrong/n <= .005 and n > accepted_best:
        threshold, accepted_best = candidate, n


def metrics(ix, probabilities=None):
    probabilities = evaluate(ix).softmax(-1) if probabilities is None else probabilities
    score,predicted = probabilities.max(-1)
    accepted = (predicted < len(domain.DOMAIN_ACTIONS)) & (score >= threshold)
    wrong = accepted & (predicted != y[ix])
    return {'count': len(ix), 'raw_correct': int((predicted == y[ix]).sum()), 'accepted_actions': int(accepted.sum()), 'wrong_accepted_actions': int(wrong.sum()), 'false_executions': int((wrong & (y[ix] >= len(domain.DOMAIN_ACTIONS))).sum())}
save_file(best, str(args.output/'typed.safetensors'))
manifest = {'version': 2, 'labels': domain.DOMAIN_LABELS, 'base_sha256': domain.file_hash(args.model/'model.safetensors'), 'typed_sha256': domain.file_hash(args.output/'typed.safetensors'), 'question': question, 'threshold': threshold, 'splits': {'calibration': metrics(indices['calibration'], cal), 'test': metrics(indices['test'])}, 'trained_encoder': True, 'execution': 'none', 'corpus_sha256': domain.file_hash(args.library)}
(args.output/'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n')
print(json.dumps(manifest), flush=True)

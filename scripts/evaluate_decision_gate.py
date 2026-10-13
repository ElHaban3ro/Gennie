#!/usr/bin/env python3
"""Contrasta el filtro semántico con predicciones conservadas; no ejecuta efectos."""
from pathlib import Path

# Mismo CLI y constructor de grupos que entrenamiento; no entrena ni cambia pesos.
root_path = Path(__file__).resolve().parents[1]
exec((root_path/'scripts/train_decisions.py').read_text().split('if args.finetune:')[0])
import laya

agent = laya.load(str(args.model), device='mps')
adapter = domain.DomainAdapter(agent, args.output, domain.file_hash(args.model/'model.safetensors'))
records = json.loads((args.output/'calibration-predictions.json').read_text())
requests = {sample['id']:sample['request'] for sample in samples}
report = {'execution':'none','context':'provided_fixture','guard_threshold':.9,'partitions':{}}
for name, rows in records.items():
    accepted = []
    for row in rows:
        label = row['label']
        threshold = adapter.metadata.get('action_thresholds',{}).get(label,adapter.threshold)
        if label not in domain.DOMAIN_ACTIONS or label in adapter.metadata.get('disabled_actions',[]) or row['probability'] < threshold:
            continue
        result = dict(row)
        result['guard_accepted'] = adapter.check_semantics(requests[row['id']],label)
        accepted.append(result)
        if len(accepted)%20 == 0:
            print(f'{name}: {len(accepted)} candidatos comprobados',flush=True)
    allowed = [r for r in accepted if r['guard_accepted']]
    report['partitions'][name] = {'count':len(rows),'accepted_actions':len(allowed),
        'wrong_accepted_actions':sum(r['truth']!=r['label'] for r in allowed),
        'false_executions':sum(r['truth'] not in domain.DOMAIN_ACTIONS for r in allowed),
        'results':accepted}
    (args.output/'gate-evaluation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({name:{k:v for k,v in report['partitions'][name].items() if k!='results'}}),flush=True)

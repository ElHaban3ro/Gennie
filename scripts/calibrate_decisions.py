#!/usr/bin/env python3
"""Calibra por acción usando solo calibración; compara posteriormente con la prueba."""
# Reutiliza exactamente el constructor de corpus/grupos del entrenamiento, sin reentrenar.
from pathlib import Path
root_path = Path(__file__).resolve().parents[1]
source = (root_path/'scripts/train_decisions.py').read_text().split('if args.finetune:')[0]
exec(source)
import laya
agent = laya.load(str(args.model), device='mps')
adapter = domain.DomainAdapter(agent, args.output, domain.file_hash(args.model/'model.safetensors'))
original = dict(adapter.metadata)
adapter.threshold = 0
adapter.metadata['action_thresholds'] = {}
adapter.metadata['disabled_actions'] = []


def predict_partition(name):
    results = []
    for start in range(0,len(indices[name]),8):
        selected = indices[name][start:start+8]
        predicted = adapter.infer([samples[i]['request'] for i in selected])
        for index, result in zip(selected,predicted):
            results.append({'id':samples[index]['id'],'truth':samples[index]['label'], 'label':result['action'] if result['route']=='complete' else result['route'], 'probability':result['probability']})
        if start % 160 == 0:
            print(f'{name} {start}/{len(indices[name])}',flush=True)
    return results
calibration = predict_partition('calibration')
thresholds,disabled = {},[]
for action in domain.DOMAIN_ACTIONS:
    rows = [row for row in calibration if row['label']==action]
    accepted_best,threshold = 0,1.
    for candidate in sorted(set([0.,1.] + [r['probability'] for r in rows])):
        accepted = [r for r in rows if r['probability'] >= candidate]
        if len(accepted) >= 20 and not any(r['truth']!=action for r in accepted) and len(accepted)>accepted_best:
            accepted_best,threshold = len(accepted),candidate
    thresholds[action] = threshold
    if accepted_best == 0:
        disabled.append(action)
test = predict_partition('test')


def metrics(rows):
    accepted = [row for row in rows if row['label'] in domain.DOMAIN_ACTIONS and row['label'] not in disabled and row['probability']>=thresholds[row['label']]]
    wrong = [row for row in accepted if row['truth']!=row['label']]
    return {'count':len(rows),'accepted_actions':len(accepted),'wrong_accepted_actions':len(wrong),'false_executions':sum(row['truth'] not in domain.DOMAIN_ACTIONS for row in wrong),'raw_correct':sum(row['truth']==row['label'] for row in rows)}
original['action_thresholds'] = thresholds
original['disabled_actions'] = disabled
original['splits_per_action'] = {'calibration':metrics(calibration),'test':metrics(test)}
original['global_splits'] = original['splits']
original['splits'] = original['splits_per_action']
(args.output/'manifest.json').write_text(json.dumps(original,ensure_ascii=False,indent=2)+'\n')
(args.output/'calibration-predictions.json').write_text(json.dumps({'calibration':calibration,'test':test},ensure_ascii=False,indent=2)+'\n')
print(json.dumps(original['splits']),flush=True)

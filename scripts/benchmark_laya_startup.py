#!/usr/bin/env python3
"""Mide arranque real y compara pesos/predicciones; sin micrófono ni efectos."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--baseline', action='store_true')
    parser.add_argument('--reference', type=Path)
    parser.add_argument('--max-ready-seconds', type=float)
    args = parser.parse_args()
    os.environ.update(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1',
                      USE_TF='0', TOKENIZERS_PARALLELISM='false')
    started = time.perf_counter()
    spec = importlib.util.spec_from_file_location(
        'worker', Path(__file__).resolve().parents[1]/'apps/decisions/python/worker.py')
    worker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(worker)
    if args.baseline:
        import torch
        import laya
        torch.set_num_threads(2)
        agent = laya.load(str(args.model), device='mps')
    else:
        agent = worker.load_local_agent(args.model)
    worker.choice(agent, 'No hay ninguna petición de usuario.', 'warmup',
                  {'idle': 'Sin petición.', 'request': 'Hay una petición.'},
                  'Clasifica el estado de inicio.')
    ready_seconds = time.perf_counter()-started
    tensors = {}
    for name, tensor in list(agent.model.named_parameters())+list(agent.model.named_buffers()):
        tensors[name] = hashlib.sha256(
            tensor.detach().cpu().contiguous().numpy().tobytes()).hexdigest()
    context = worker.system_context(read_players=False)
    cases = ['Por favor, abre Discord.', 'Abre Safari.', 'Nunca abras Discord.',
             'No quiero que abras Discord.', 'Abre Discord y Safari.', 'Ayer abrí Discord.']
    predictions = [worker.decide(agent, {'text': text}, context) for text in cases]
    report = {'mode': 'baseline' if args.baseline else 'optimized',
              'execution': 'none', 'ready_seconds': ready_seconds,
              'device': str(agent.device), 'dtype': str(agent.dtype),
              'tensors': tensors, 'cases': cases, 'predictions': predictions}
    passed = True
    if args.reference:
        reference = json.loads(args.reference.read_text())
        report['identical_tensors'] = tensors == reference['tensors']
        report['identical_predictions'] = (cases == reference['cases']
                                           and predictions == reference['predictions'])
        passed = report['identical_tensors'] and report['identical_predictions']
    if args.max_ready_seconds is not None:
        report['within_time_budget'] = ready_seconds <= args.max_ready_seconds
        passed = passed and report['within_time_budget']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({key: value for key, value in report.items()
                      if key not in ('tensors', 'cases', 'predictions')}))
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())

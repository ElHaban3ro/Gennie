#!/usr/bin/env python3
"""Comprueba integridad CSV/JSONL y separación de grupos; sin inferencia ni efectos."""
import argparse
import csv
import importlib.util
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--jsonl', type=Path, required=True)
parser.add_argument('--csv', type=Path, required=True)
parser.add_argument('--split', type=Path)
args = parser.parse_args()
rows = [json.loads(line) for line in args.jsonl.read_text().splitlines()]
with args.csv.open() as file:
    csv_rows = {row['id']: row for row in csv.DictReader(file)}
assert len(rows) == len(csv_rows), 'Número de filas diferente'
assert len({row['id'] for row in rows}) == len(rows), 'IDs JSONL duplicados'
assert {row['id'] for row in rows} == set(csv_rows), 'IDs CSV/JSONL diferentes'
for row in rows:
    counterpart = csv_rows[row['id']]
    assert counterpart['utterance'] == row['state']['utterance']
    assert counterpart['expected_action'] == row['expected_action']
if args.split:
    split = json.loads(args.split.read_text())
    groups = {}
    for sample in split['samples']:
        assert sample['split'] in ('train','calibration','test')
        assert groups.setdefault(sample['group'],sample['split']) == sample['split'], 'Grupo repetido entre particiones'
print(json.dumps({'cases':len(rows),'integrity':True,'groups_disjoint':bool(args.split),'model_inference':False,'execution':False}))

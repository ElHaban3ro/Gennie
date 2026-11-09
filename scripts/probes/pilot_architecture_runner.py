#!/usr/bin/env python3
"""Una celda aislada de la arquitectura experimental; no es el constructor de producción."""
import argparse
import json
from pathlib import Path
import time
import uuid

import assignment_evaluation as common
import pilot_architecture
import pilot_fixture
import pilot_session

ROOT = Path(__file__).resolve().parents[2]
MAX_ROLE_CALLS = 10
SOURCES = [Path(__file__).resolve(),Path(pilot_session.__file__).resolve(),Path(pilot_architecture.__file__).resolve(),
           Path(pilot_fixture.__file__).resolve(),Path(pilot_session.worker.__file__).resolve(),pilot_session.SERVER]


def run_cell(case, *, mode, directory, session=pilot_session.call):
    if mode not in ('protocol','live') or case not in pilot_fixture.CASES:
        raise ValueError('invalid_scope')
    directory = Path(directory)
    directory.mkdir(parents=True,exist_ok=False,mode=0o700)
    fingerprints = {str(path.relative_to(ROOT)):common.sha_file(path) for path in SOURCES}
    fingerprints['codex_binary'] = common.sha_file(Path(common.codex_binary()))
    common.save(directory/'plan.json',{'case':case,'mode':mode,'max_role_calls':MAX_ROLE_CALLS,
                                     'max_attempts':4,'model':'gpt-6.1-terra','effort':'medium',
                                     'per_role_timeout_seconds':60,'real_external_effects':False,'fingerprints':fingerprints})
    records = []
    started = time.monotonic()
    if mode == 'protocol':
        sample = session(case,'planner',{'request':pilot_fixture.CASES[case]['request']},mode='protocol',model='gpt-6.1-terra',effort='medium')
        records.append(sample)
        outcome = {'status':'protocol_verified' if sample['result']['status']=='protocol_verified' and not sample['calls'] and sample.get('temporary_removed') is True else 'protocol_failed'}
    else:
        def agent(role,payload):
            if len(records) >= MAX_ROLE_CALLS:
                raise pilot_architecture.PilotError('role_budget_exhausted')
            sample = session(case,role,payload,mode='live',model='gpt-6.1-terra',effort='medium')
            records.append({'role':role,**sample})
            common.save(directory/f'role-{len(records):02d}.json',records[-1])
            if sample['result']['status'] != 'completed' or not isinstance(sample['response'],dict) or sample.get('temporary_removed') is not True:
                raise pilot_architecture.PilotError('role_failed_or_invalid')
            required = {'planner':'inspect_page','executor':'test_recipe','parent_validator':'test_recipe'}.get(role)
            if required and not any(item.get('tool') == required for item in sample['calls']):
                raise pilot_architecture.PilotError('required_tool_not_observed')
            return sample['response']
        try:
            outcome = pilot_architecture.run(case,agent,max_attempts=4)
        except pilot_architecture.PilotError as error:
            outcome = {'status':str(error)}
    totals = [record['result'].get('usage_reported',{}).get('total') if isinstance(record['result'].get('usage_reported'),dict) else None for record in records]
    fields = ('inputTokens','cachedInputTokens','cacheWriteInputTokens','outputTokens','reasoningOutputTokens','totalTokens')
    aggregate = {field:sum(t[field] for t in totals) if totals and all(isinstance(t,dict) and type(t.get(field)) is int for t in totals) else None for field in fields}
    drift = {key:common.sha_file(Path(common.codex_binary()) if key == 'codex_binary' else ROOT/key) != value for key,value in fingerprints.items()}
    report = {'case':case,'mode':mode,'outcome':outcome,'roles':len(records),'usage_total':aggregate,'source_drift':any(drift.values()),
              'usage_complete':bool(totals) and all(isinstance(t,dict) and type(t.get('totalTokens')) is int for t in totals),
              'elapsed_ms':round((time.monotonic()-started)*1000),'cost':None}
    common.save(directory/'result.json',report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case',required=True,choices=tuple(pilot_fixture.CASES))
    parser.add_argument('--mode',required=True,choices=('protocol','live'))
    args = parser.parse_args()
    path = ROOT/'.verification/architecture-pilot'/str(uuid.uuid4())
    result = run_cell(args.case,mode=args.mode,directory=path)
    print(json.dumps({'directory':str(path),'outcome':result['outcome']['status'],'roles':result['roles'],
                      'usage_complete':result['usage_complete']},ensure_ascii=False))


if __name__ == '__main__':
    main()

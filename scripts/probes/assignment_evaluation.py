#!/usr/bin/env python3
"""Prepara, ejecuta y resume el experimento; ninguna inferencia sin run explícito."""
import argparse
import collections
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid
from assignment_evaluation_worker import binary as codex_binary

ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT/'bootstrap/tests/fixtures/assignment-evaluation-v1.json'
PILOT = ['duplicate_scope','ambiguous_source','insufficient_permission','adversarial_context']


def read(path):
    return json.loads(path.read_text())


def save(path, value):
    with path.open('x', encoding='utf-8') as file:
        os.chmod(path, 0o600)
        json.dump(value, file, ensure_ascii=False, indent=2)
        file.write('\n')


def fingerprint():
    paths = {CORPUS, ROOT/'Cargo.lock', ROOT/'Cargo.toml'}
    crates = [ROOT/'bootstrap']
    for area in ('apps', 'kernel'):
        crates.extend(p.parent for p in (ROOT/area).glob('*/Cargo.toml'))
    # Incluye módulos extraídos/añadidos, manifiestos y SQL, sin perfiles/env/target.
    for crate in crates:
        manifest = crate/'Cargo.toml'
        if manifest.is_file(): paths.add(manifest)
        paths.update(p for p in (crate/'src').rglob('*') if p.is_file() and p.suffix in ('.rs', '.md', '.mjs'))
        paths.update((crate/'migrations').glob('*.sql'))
    paths.update((ROOT/'scripts/probes').glob('assignment_evaluation*.py'))
    return {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}


def sha_file(path):
    digest=hashlib.sha256()
    with Path(path).open('rb') as file:
        for block in iter(lambda:file.read(1024*1024),b''):digest.update(block)
    return digest.hexdigest()


def plan_digest(plan):
    return hashlib.sha256(json.dumps(plan,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()


def binaries():
    paths={'helper':ROOT/'target/flutter/debug/gennie','codex':Path(codex_binary())}
    return {k:{'path':str(p),'sha256':sha_file(p) if p.is_file() else None} for k,p in paths.items()}


def prepare(args):
    corpus = read(CORPUS)
    if args.scope in ('submission', 'smoke') and args.repetitions != 1:
        raise SystemExit('La comprobación breve admite una repetición; prepara una comparación aparte.')
    cases = ['duplicate_scope'] if args.scope in ('submission','smoke') else PILOT if args.scope == 'pilot' else [c['id'] for c in corpus['cases'] if c['family'] == 'proposals']
    requested = getattr(args, 'cases', None)
    if requested is not None:
        if args.scope != 'full' or not requested or len(set(requested)) != len(requested) or set(requested) - set(cases):
            raise SystemExit('--case requiere scope full y casos semánticos conocidos, únicos y no vacíos.')
        cases = [case for case in cases if case in requested]
    run_id = str(uuid.uuid4())
    directory = ROOT/'.verification/assignment-evaluation'/run_id
    directory.mkdir(parents=True, mode=0o700)
    representations = ['flat_v1'] if args.scope == 'submission' else ['flat_v1','grouped_v1']
    strategies = (['local_rules','bot_guide' if args.with_guide else 'bot'] if args.scope == 'submission'
                  else ['local_rules','bot']+(['bot_guide'] if args.with_guide else []))
    model_cells = len(cases)*len(representations)*args.repetitions*(len(strategies)-1)
    plan = {
        'version':1,'mode':'live','output_dir':str(directory),'cases':cases,'strategies':strategies,
        'representations':representations,'repetitions':args.repetitions,
        'model':'gpt-6-luna','effort':'low','cell_timeout_seconds':60,
        'total_timeout_seconds':args.budget_minutes*60,
        'helper':str(ROOT/'target/flutter/debug/gennie'),
        'worker':str(ROOT/'scripts/probes/assignment_evaluation_worker.py'),
        'guide_environment':str(Path(args.guide_env).resolve()) if args.guide_env else None,
        'metadata':{'run_id':run_id,'corpus_version':corpus['version'],'seed':corpus['seed'],'scope':args.scope,'stop_on_failed_model_cell':args.scope in ('submission','smoke'),'model_cells_max':model_cells,'guide_calls_max':len(cases)*len(representations)*args.repetitions if args.with_guide else 0,'source_sha256':fingerprint(),'binaries':binaries(),'currency_cost_budget':None,'human_quality_scores':'pending','production_storage_changes':False},
    }
    save(directory/'plan.json', plan)
    save(directory/'READY.json',{'plan_sha256':plan_digest(plan),'execution_started':False})
    representation_label = 'plana vigente, sin comparar formatos' if args.scope == 'submission' else 'plana vigente y agrupada, con los mismos datos'
    lines = [f'# Plan de evaluación {run_id}', '', f'Corpus sintético v{corpus["version"]}, semilla {corpus["seed"]}.', '', f'- Casos: {", ".join(cases)}.', f'- Representaciones: {representation_label}.', f'- Estrategias: {", ".join(strategies)}; repeticiones: {args.repetitions}.', f'- Máximo de turnos visibles del bot: {model_cells}; consultas de guía: {plan["metadata"]["guide_calls_max"]}.', f'- Presupuesto de ejecución: {args.budget_minutes} minutos; cada celda dispone de hasta 60 segundos, más cierre acotado.', '- Sin permisos de archivos, comandos, aplicaciones, búsqueda web ni escritura de planes.', '- No usa encargos personales ni cambia el almacenamiento del producto.', '- Coste monetario desconocido: se conservará lo reportado, sin tratar ausencias como cero.', '- Fallos, abstenciones y celdas no ejecutadas se conservan; no hay reintento automático.', '', 'Antes de ejecutar live requiere aprobación del alcance según docs/rules/testing.md.', 'El identificador del comando evita ejecuciones accidentales; no sustituye la autorización del usuario.', '', f'Comando después de autorizar: `python3 scripts/probes/assignment_evaluation.py run {directory} --ack {run_id}`', '']
    if args.scope in ('submission','smoke'):
        lines += ['## Comprobación breve del contrato', '',
            f'Modelo del bot: {plan["model"]}, esfuerzo {plan["effort"]}. Guía: ' + ('openai/gpt-6-luna-decisions mediante OpenRouter.' if args.with_guide else 'sin consulta remota.'),
            'Comprueba envío válido de candidatos y, cuando se consiente, una valoración de guía por celda. No compara calidad general ni elige formato ganador.',
            'El bot puede corregir argumentos rechazados antes del guardado dentro del mismo turno autorizado. No se inicia otro turno ni se repite una consulta de guía fallida o incierta.',
            'Se detiene al primer fallo de una celda del modelo, ausencia de borrador, comprobación automática fallida o guía no confirmada. Las restantes quedan registradas sin ejecutar.',
            'Codex debe descubrir exactamente los esquemas del host antes de cada turno; una diferencia impide generar.', '']
    (directory/'PLAN.md').write_text('\n'.join(lines))
    print(json.dumps({'directory':str(directory),'run_id':run_id,'model_turns_max':model_cells,'guide_calls_max':plan['metadata']['guide_calls_max'],'execution_started':False}, ensure_ascii=False))


def result_artifacts_match(directory, plan):
    """Integridad del recorrido; no convierte fallos semánticos en éxitos."""
    try:
        manifest=read(directory/f'{plan["mode"]}-manifest.json')
        filename={'offline':'baseline','protocol':'protocol','live':'live'}[plan['mode']]+'.jsonl'
        rows=[json.loads(line) for line in (directory/filename).read_text().splitlines()]
        corpus={case['id']:case for case in read(CORPUS)['cases']}
        expected=collections.Counter()
        for repetition in range(1,plan['repetitions']+1):
            for case_id in plan['cases']:
                if corpus[case_id]['family'] == 'contract':
                    expected[(case_id,'contract_control',None,None)]+=1
                else:
                    for representation in plan['representations']:
                        for strategy in plan['strategies']:
                            expected[(case_id,strategy,representation,repetition)]+=1
        actual=collections.Counter()
        for row in rows:
            if not isinstance(row,dict) or not isinstance(row.get('status'),str) or not row['status']:
                return False
            actual[(row.get('case_id'),row.get('strategy'),row.get('representation'),row.get('repetition'))]+=1
            if plan['mode'] != 'live':
                guide_key='remote_guide_calls' if row.get('family') == 'contract' else 'guide_calls'
                for key in ('model_turns',guide_key):
                    if type(row.get(key)) is not int or row[key] != 0:
                        return False
            if plan['mode'] == 'protocol':
                provider=row.get('provider',{})
                schemas=provider.get('schema_inventory',{})
                if (row['status'] != 'protocol_verified' or provider.get('status') != 'protocol_verified'
                        or type(provider.get('generation_turns')) is not int or provider['generation_turns'] != 0
                        or not isinstance(schemas,dict) or not schemas
                        or any(not isinstance(value,dict) or value.get('matches') is not True for value in schemas.values())):
                    return False
            if row.get('repetition') is not None and type(row['repetition']) is not int:
                return False
        sequences=sum(count for key,count in expected.items() if key[1] != 'contract_control')
        return (bool(rows) and actual == expected and type(manifest.get('version')) is int and manifest['version'] == 1
                and manifest.get('mode') == plan['mode']
                and type(manifest.get('sequence_count')) is int and manifest['sequence_count'] == sequences
                and plan_digest(manifest.get('metadata')) == plan_digest(plan['metadata']))
    except (OSError,ValueError,KeyError,TypeError,AttributeError):
        return False


def execute(directory, mode, ack=None):
    if mode not in ('offline','protocol','live'):
        raise SystemExit('Modo de evaluación no admitido.')
    directory = Path(directory).resolve()
    if (directory/'CLOSURE.json').exists():
        raise SystemExit('El plan está cerrado; requiere otra preparación.')
    plan = read(directory/'plan.json')
    if read(directory/'READY.json')['plan_sha256'] != plan_digest(plan):
        raise SystemExit('El alcance o presupuesto del plan cambió; prepara otro plan antes de ejecutar.')
    if plan['metadata']['source_sha256'] != fingerprint():
        raise SystemExit('El código o corpus cambió: prepara otro plan; no se reutiliza una comparación distinta.')
    if mode == 'live':
        if ack != plan['metadata']['run_id']:
            raise SystemExit('Falta --ack con el identificador exacto del plan autorizado.')
        if plan['metadata']['binaries'] != binaries():
            raise SystemExit('El helper o Codex cambió; prepara otro plan con los binarios actuales.')
    else:
        plan['mode'] = mode
        plan['repetitions'] = 1
        plan['guide_environment'] = None
        if mode == 'offline':
            plan['cases'] = [c['id'] for c in read(CORPUS)['cases']]
            plan['strategies'] = ['local_rules']
        else:
            plan['cases'] = [PILOT[0]]
            plan['strategies'] = ['bot']
            plan['representations'] = ['flat_v1']
            plan['cell_timeout_seconds'] = 30
            plan['total_timeout_seconds'] = 60
    invocation = directory/f'{mode}-invocation.json'
    save(invocation, plan)
    env = os.environ.copy()
    env['GENNIE_TEST_EVALUATION_PLAN'] = str(invocation)
    with (directory/f'{mode}.log').open('x') as log:
        process = subprocess.run(['cargo','test','-p','gennie','--locked','assignment_evaluation_run','--','--ignored','--nocapture'], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=False)
    drift = plan['metadata']['source_sha256'] != fingerprint()
    raw=directory/({'offline':'baseline','protocol':'protocol','live':'live'}[mode]+'.jsonl')
    valid=result_artifacts_match(directory,plan)
    save(directory/f'{mode}-completion.json', {'exit_code':process.returncode,'source_drift':drift,'results_preserved':raw.is_file(),'result_contract_passed':valid,'raw_sha256':sha_file(raw) if raw.exists() else None})
    report(directory)
    if process.returncode or drift or not valid:
        raise SystemExit('El recorrido falló o cambió el origen. Los resultados y fallos se conservaron; consulta el log.')


def report(directory):
    directory = Path(directory).resolve()
    plan = read(directory/'plan.json')
    corpus = read(CORPUS)
    cases = {c['id']:c for c in corpus['cases']}
    rows = []
    malformed = []
    for source in ('baseline.jsonl','protocol.jsonl','live.jsonl'):
        path = directory/source
        if not path.exists():
            continue
        completion=directory/({'baseline.jsonl':'offline','protocol.jsonl':'protocol','live.jsonl':'live'}[source]+'-completion.json')
        if completion.exists() and read(completion).get('raw_sha256') not in (None,sha_file(path)):
            raise SystemExit('Un registro bruto terminado cambió; se conservan las valoraciones y no se regeneran paquetes sobre otros datos.')
        for number,line in enumerate(path.read_text().splitlines(),1):
            try:
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise ValueError()
            except ValueError:
                malformed.append({'file':source,'line':number})
                continue
            row['source'] = source
            row['source_line'] = number
            rows.append(row)
    aggregate = collections.Counter((r.get('source'),r.get('strategy'),r.get('representation'),r.get('status')) for r in rows)
    checks_failed = []
    for index,row in enumerate(rows):
        checks = row.get('machine',{}).get('checks',row.get('checks',{}))
        for name,value in checks.items():
            if value is False:
                checks_failed.append({'row':index,'case_id':row.get('case_id'),'check':name})
    unknown_model_counts = sum(r.get('model_turns') is None for r in rows)
    summary = {'run_id':plan['metadata']['run_id'],'rows':len(rows),'statuses':[{'source':k[0],'strategy':k[1],'representation':k[2],'status':k[3],'count':v} for k,v in sorted(aggregate.items(), key=lambda item:str(item[0]))],'machine_checks_failed':checks_failed,'malformed_rows':malformed,'visible_model_turns':sum(r.get('model_turns') or 0 for r in rows),'rows_with_unknown_model_count':unknown_model_counts,'semantic_quality':None,'monetary_cost_total':None,'comparison_conclusion':'pending_model_cells_and_review'}
    (directory/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
    blind = []
    mapping = []
    for index,row in enumerate(rows):
        if row.get('family') == 'contract' or row.get('source') == 'protocol.jsonl':
            continue
        identifier = hashlib.sha256(f'{plan["metadata"]["run_id"]}:{row["source"]}:{row["source_line"]}'.encode()).hexdigest()[:12]
        case = cases[row['case_id']]
        blind.append({'blind_id':identifier,'case_id':row['case_id'],'request':case['request'],'rubric':case['rubric'],'input_documents':row.get('input_documents'),'proposals':row.get('proposals'),'messages':row.get('provider',{}).get('messages'),'output_available':bool(row.get('proposals') or row.get('provider',{}).get('messages'))})
        mapping.append({'blind_id':identifier,'row':index,'case_id':row.get('case_id'),'strategy':row.get('strategy'),'representation':row.get('representation'),'repetition':row.get('repetition')})
    (directory/'review-packet.json').write_text(json.dumps(sorted(blind,key=lambda r:r['blind_id']),ensure_ascii=False,indent=2)+'\n')
    (directory/'review-key.json').write_text(json.dumps(mapping,ensure_ascii=False,indent=2)+'\n')
    ratings = directory/'ratings.csv'
    columns=['blind_id','reviewer','reviewer_kind','usefulness_0_to_2','clarity_0_to_2','questions_0_to_2','scope_preserved','decision','duplicate_classification','notes']
    existing=[]
    if ratings.exists():
        with ratings.open(newline='') as file:
            reader=csv.DictReader(file)
            if reader.fieldnames != columns:
                raise SystemExit('El formato de ratings.csv difiere; se conserva el archivo, sin reemplazar valoraciones.')
            existing=list(reader)
    known={r['blind_id'] for r in existing}
    with ratings.open('a' if ratings.exists() else 'x',newline='') as file:
        writer=csv.writer(file)
        if not existing and not known and file.tell()==0:writer.writerow(columns)
        for row in sorted(blind,key=lambda r:r['blind_id']):
            if row['blind_id'] not in known:writer.writerow([row['blind_id']]+['']*(len(columns)-1))
    rated, errors, seen = [], [], set()
    by_id={r['blind_id']:r for r in mapping}
    for row in existing:
        if not row['reviewer']:
            continue
        key=(row['blind_id'],row['reviewer'],row['reviewer_kind'])
        valid=row['blind_id'] in by_id and row['reviewer_kind'] in ('human','ai_assisted','unspecified') and key not in seen
        scores={k:None if row[k]=='' else int(row[k]) if row[k] in ('0','1','2') else 'invalid' for k in columns[3:6]}
        if not valid or 'invalid' in scores.values() or row['scope_preserved'] not in ('','yes','no') or row['decision'] not in ('','accept_without_edit','accept_after_edit','reject','unrateable') or row['duplicate_classification'] not in ('','correct_duplicate','missed_duplicate','false_duplicate','not_applicable'):
            errors.append({'blind_id':row['blind_id'],'code':'invalid_or_duplicate_rating'});continue
        seen.add(key)
        rated.append({**by_id[row['blind_id']], 'reviewer':row['reviewer'],'reviewer_kind':row['reviewer_kind'],'scores':scores,'scope_preserved':row['scope_preserved'] or None,'decision':row['decision'] or None,'duplicate_classification':row['duplicate_classification'] or None})
    paired=[]
    for row in rated:
        if row['representation']!='flat_v1':continue
        other=next((r for r in rated if r['representation']=='grouped_v1' and all(r[k]==row[k] for k in ('case_id','strategy','repetition','reviewer','reviewer_kind'))),None)
        if other:
            paired.append({'case_id':row['case_id'],'strategy':row['strategy'],'reviewer_kind':row['reviewer_kind'],'deltas_grouped_minus_flat':{k:None if row['scores'][k] is None or other['scores'][k] is None else other['scores'][k]-row['scores'][k] for k in row['scores']}})
    summary['ratings']={'submitted':len(rated),'complete':sum(all(v is not None for v in r['scores'].values()) and r['scope_preserved'] is not None and r['decision'] is not None for r in rated),'invalid':errors,'paired':paired,'decisions':dict(collections.Counter(r['decision'] for r in rated if r['decision'])),'duplicate_classifications':dict(collections.Counter(r['duplicate_classification'] for r in rated if r['duplicate_classification'])),'reviewer_identity_verified':False}
    if rated:summary['comparison_conclusion']='ratings_available_no_general_winner_inferred'
    (directory/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
    lines = ['# Estado de la evaluación', '', f'Run: `{summary["run_id"]}`. Calidad semántica y coste total: pendientes; no hay ganador demostrado.', '', '| Origen | Estrategia | Representación | Estado | Cantidad |','|---|---|---|---|---:|']
    lines += [f'| {r["source"]} | {r["strategy"]} | {r["representation"]} | {r["status"]} | {r["count"]} |' for r in summary['statuses']]
    lines += ['', f'Fallos de comprobaciones automáticas: {len(checks_failed)}. Filas incompletas/malformadas: {len(malformed)}.', 'Una abstención no acredita utilidad ni preservación hipotética de permisos. Null es dato desconocido.', '', '`review-packet.json` oculta etiquetas de variante/modelo; el contenido puede permitir inferirlas. `review-key.json` permite desvelarlas después.', '`ratings.csv` empieza sin puntuaciones: no se rellenan con ceros ni se presentan revisiones de IA como humanas.', 'Los resultados brutos no se reemplazan al repetir report; las valoraciones existentes se conservan.', '']
    (directory/'REPORT.md').write_text('\n'.join(lines))
    print(json.dumps(summary,ensure_ascii=False))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    commands=parser.add_subparsers(dest='command',required=True)
    p=commands.add_parser('prepare');p.add_argument('--scope',choices=['submission','smoke','pilot','full'],default='pilot');p.add_argument('--case',dest='cases',action='append',help='Caso semántico del corpus; repetible y solo con --scope full.');p.add_argument('--with-guide',action='store_true');p.add_argument('--guide-env');p.add_argument('--repetitions',type=int,choices=[1,2,3],default=1);p.add_argument('--budget-minutes',type=int,choices=range(1,61),default=12)
    for name in ('baseline','protocol','run','report'):
        p=commands.add_parser(name);p.add_argument('directory');
        if name=='run':p.add_argument('--ack',required=True)
    args=parser.parse_args()
    if args.command=='prepare':prepare(args)
    elif args.command=='report':report(args.directory)
    else:execute(args.directory,{'baseline':'offline','protocol':'protocol','run':'live'}[args.command],getattr(args,'ack',None))


if __name__=='__main__':
    main()

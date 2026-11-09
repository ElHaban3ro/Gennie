#!/usr/bin/env python3
"""Sonda optativa del adaptador conversacional; preparar/protocolo no infieren."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import tarfile
import uuid
import assignment_evaluation as common
import continuity_baseline

ROOT = Path(__file__).resolve().parents[2]
SCENARIOS = {'full':4, 'stop_resume':2, 'plugin_semantics':3, 'continuity':2, 'continuity_extended':2, 'continuity_explicit':2, 'continuity_stall':2, 'continuity_correction':2, 'continuity_compare':4, 'continuity_model_compare':2, 'continuity_compare_followup':4, 'continuity_model_followup':2, 'continuity_correction_model':2}

CONTINUITY_CASES = {
    'continuity': {'two_sources':'two_sources','fallback':'fallback'},
    'continuity_extended': {'polling':'polling','delegation':'delegation'},
    'continuity_explicit': {'explicit_two_sources':'two_sources','explicit_polling':'polling'},
    'continuity_stall': {'stalled':'stalled','explicit_delegation':'delegation'},
    'continuity_correction': {'correction':'correction'},
    'continuity_compare': {'compare_a1':'two_sources','compare_b1':'two_sources','compare_b2':'two_sources','compare_a2':'two_sources'},
    'continuity_model_compare': {'compare_a1':'two_sources','compare_a2':'two_sources'},
    'continuity_compare_followup': {'compare_b3':'two_sources','compare_a3':'two_sources','compare_a4':'two_sources','compare_b4':'two_sources'},
    'continuity_model_followup': {'compare_a3':'two_sources','compare_a4':'two_sources'},
    'continuity_correction_model': {'correction':'correction'},
}


def validate_plan(plan, directory):
    fields = {'version','scenario','mode','output_dir','nonce','model','effort',
              'model_turns_max','cell_timeout_seconds'}
    if not isinstance(plan,dict) or set(plan) != fields:
        raise SystemExit('Campos del plan de sonda no admitidos.')
    scenario = plan['scenario']
    if (type(plan['version']) is not int or plan['version'] != 2
            or type(scenario) is not str or scenario not in SCENARIOS
            or plan['mode'] != 'live' or plan['model'] != ('gpt-6.1-sol' if scenario in ('continuity_model_compare','continuity_model_followup','continuity_correction_model') else 'gpt-6-luna')
            or plan['effort'] != 'low'
            or type(plan['model_turns_max']) is not int
            or plan['model_turns_max'] != SCENARIOS[scenario]
            or type(plan['cell_timeout_seconds']) is not int
            or plan['cell_timeout_seconds'] != 60
            or type(plan['nonce']) is not str
            or re.fullmatch('[a-fA-F0-9]{32}',plan['nonce']) is None
            or type(plan['output_dir']) is not str
            or not Path(plan['output_dir']).is_absolute()
            or Path(plan['output_dir']).resolve() != directory):
        raise SystemExit('Versión, escenario, presupuesto o destino no admitidos.')


def report_matches_plan(report, plan, mode):
    if (mode not in ('protocol','live') or not isinstance(report,dict) or report.get('mode') != mode
            or report.get('scenario') != plan['scenario']
            or report.get('status') != 'completed'
            or report.get('profile_removed') is not True
            or type(report.get('guide_calls')) is not int or report['guide_calls'] != 0
            or type(report.get('model_turns')) is not int):
        return False
    if plan['scenario'] == 'plugin_semantics':
        return plugin_report_matches(report, mode)
    if plan['scenario'] in CONTINUITY_CASES:
        return continuity_report_matches(report, mode, plan['scenario'])
    if mode == 'protocol':
        data = report.get('protocol')
        return (report['model_turns'] == 0 and isinstance(data,dict)
                and data.get('account_linked') is False
                and type(data.get('generation_turns')) is int and data['generation_turns'] == 0
                and all(data.get(key) is True for key in (
                    'distinct_threads','local_fixture_contexts_isolated',
                    'recorded_active_matches','cold_process_resume_same_thread')))
    stop, resume = report.get('stop'), report.get('resume')
    if (report['model_turns'] != plan['model_turns_max']
            or not isinstance(stop,dict) or not isinstance(resume,dict)
            or not all(stop.get(key) is True for key in ('verified','cancelled','cancel_requested'))
            or type(stop.get('observed_characters')) is not int or stop['observed_characters'] <= 0
            or resume.get('same_thread') is not True):
        return False
    if plan['scenario'] == 'full':
        quote, session = report.get('quote'), report.get('new_thread')
        return (isinstance(quote,dict) and quote.get('matched') is True
                and isinstance(session,dict) and all(session.get(k) is True for k in (
                    'distinct','quote_absent_from_provider_history','literal_journal_input_preserved')))
    return True



def continuity_report_matches(report, mode, scenario='continuity'):
    if report['model_turns'] != (0 if mode == 'protocol' else SCENARIOS[scenario]):
        return False
    threads = []
    cases = CONTINUITY_CASES[scenario]
    keys = cases.keys()
    for key in keys:
        case = report.get(key)
        if (not isinstance(case,dict) or case.get('status') != 'completed'
                or case.get('schemas_exact') is not True or case.get('socket_removed') is not True
                or not isinstance(case.get('thread_id'),str) or not case['thread_id']
                or not isinstance(case.get('trace'),list)):
            return False
        threads.append(case['thread_id'])
        if mode == 'protocol' and (case['trace'] != [] or case.get('response') is not None):
            return False
        if mode == 'live':
            response = case.get('response')
            if not isinstance(response,str) or not response.strip():
                return False
            if not case['trace'] or case['trace'][-1] != {'event':'reply','text':response}:
                return False
            if key == 'correction':
                replies = [e.get('text') for e in case['trace'] if e.get('event') == 'reply']
                if (len(replies) != 2 or replies[0] != case.get('first_response')
                        or sum(e.get('event') == 'correction' for e in case['trace']) != 1):
                    return False
            try:
                continuity_baseline.evaluate(cases[key],case['trace'])
            except (ValueError,TypeError):
                return False
    if len(set(threads)) != len(threads):
        return False
    if mode == 'protocol':
        data = report.get('protocol',{})
        return (data.get('account_linked') is False and type(data.get('generation_turns')) is int
                and data['generation_turns'] == 0 and data.get('schemas_exact') is True)
    return True


def plugin_report_matches(report, mode):
    expected = {'ambiguous_recipient':'Llegaré a las 8','uncertain_route':'Recibido'}
    if report['model_turns'] != (0 if mode == 'protocol' else 3):
        return False
    for key, text in expected.items():
        case = report.get(key)
        if (not isinstance(case,dict) or case.get('status') != 'completed'
                or case.get('schemas_exact') is not True or case.get('socket_removed') is not True
                or not isinstance(case.get('thread_id'),str) or not case['thread_id']
                or case.get('behaviour_verified') is not (mode == 'live')):
            return False
        ledger = case.get('ledger')
        if (not isinstance(ledger,dict) or ledger.get('forbidden_attempt') is not False
                or type(ledger.get('run_attempts')) is not int
                or not isinstance(ledger.get('attempts'),list)
                or not isinstance(ledger.get('fictional_effects'),list)):
            return False
        runs = [call for call in ledger['attempts'] if isinstance(call,dict) and call.get('name') == 'plugin_mcp_run']
        if (any(not isinstance(call,dict) or call.get('name') not in ('plugin_mcp_catalog','plugin_mcp_run') for call in ledger['attempts'])
                or ledger['run_attempts'] != len(runs)):
            return False
        if mode == 'protocol':
            if ledger['attempts'] or ledger['fictional_effects'] or case.get('responses') != []:
                return False
        else:
            effect = {'route':'primary','arguments':{'recipient':'cata_work','message':text}}
            if (len(runs) != 1 or ledger['fictional_effects'] != [effect]
                    or common.plan_digest(runs[0].get('arguments')) != common.plan_digest({'server_id':'primary','tool':'send_message','expected_revision':1,'arguments':effect['arguments']})):
                return False
            replies=case.get('responses')
            if (not isinstance(replies,list) or len(replies) != (2 if key == 'ambiguous_recipient' else 1)
                    or any(not isinstance(reply,str) or not reply.strip() for reply in replies)):
                return False
            if key == 'ambiguous_recipient':
                first=case.get('after_first',{})
                if (first.get('run_attempts') != 0 or type(first.get('run_attempts')) is not int
                        or first.get('fictional_effects') != [] or first.get('forbidden_attempt') is not False
                        or not isinstance(first.get('attempts'),list)
                        or any(not isinstance(call,dict) or call.get('name') != 'plugin_mcp_catalog' for call in first['attempts'])):
                    return False
    if report['ambiguous_recipient']['thread_id'] == report['uncertain_route']['thread_id']:
        return False
    if mode == 'protocol':
        protocol=report.get('protocol',{})
        return (protocol.get('account_linked') is False
                and type(protocol.get('generation_turns')) is int and protocol['generation_turns'] == 0
                and protocol.get('schemas_exact') is True and protocol.get('cases_without_effects') is True)
    return True


def sources():
    result = common.fingerprint()
    for p in (ROOT/'scripts/probes').glob('chat_provider_probe*.py'):
        result[str(p.relative_to(ROOT))] = common.sha_file(p)
    for p in (ROOT/'scripts/probes').glob('continuity_baseline*.py'):
        result[str(p.relative_to(ROOT))] = common.sha_file(p)
    return result


def binaries():
    p = Path(common.codex_binary())
    helper = ROOT/'target/flutter/debug/gennie'
    return {'helper': {'path':str(helper),'sha256':common.sha_file(helper)},
            'codex': {'path': str(p), 'sha256': common.sha_file(p)},
            'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip()}


def prepare(scenario='stop_resume'):
    if scenario not in SCENARIOS:
        raise SystemExit('Escenario no admitido.')
    turns = SCENARIOS[scenario]
    run_id = str(uuid.uuid4())
    directory = ROOT/'.verification/chat-provider'/run_id
    directory.mkdir(parents=True, mode=0o700)
    plan = {'version': 2, 'scenario':scenario, 'mode': 'live', 'output_dir': str(directory),
            'nonce': uuid.uuid4().hex, 'model': 'gpt-6.1-sol' if scenario in ('continuity_model_compare','continuity_model_followup','continuity_correction_model') else 'gpt-6-luna', 'effort': 'low',
            'model_turns_max': turns, 'cell_timeout_seconds': 60}
    validate_plan(plan,directory)
    seal = {'run_id': run_id, 'plan_sha256': common.plan_digest(plan),
            'sources': sources(), 'binaries': binaries(), 'live_started': False}
    common.save(directory/'plan.json', plan)
    archive = directory/'sources.tar.gz'
    with archive.open('xb') as raw:
        os.fchmod(raw.fileno(),0o600)
        with tarfile.open(fileobj=raw,mode='w:gz') as tar:
            for relative, expected in seal['sources'].items():
                source = ROOT/relative
                if common.sha_file(source) != expected:
                    raise SystemExit('Fuentes cambiaron durante preparación; no ejecutar este plan.')
                info = tar.gettarinfo(str(source),arcname=relative)
                info.mode = 0o600
                with source.open('rb') as f: tar.addfile(info,f)
    seal['archive_sha256'] = common.sha_file(archive)
    common.save(directory/'READY.json', seal)
    title = {'stop_resume':'Detener y reanudar','full':'Citas, sesión nueva y Detener','plugin_semantics':'Destinatario ambiguo y ruta alternativa incierta','continuity':'Continuidad de lecturas y recuperación','continuity_extended':'Espera y delegación parcial','continuity_explicit':'Herramienta nombrada explícitamente','continuity_stall':'Estado invariable y delegación parcial','continuity_correction':'Corrección de objetivo','continuity_compare':'Comparación controlada de contexto','continuity_model_compare':'Comparación de modelo','continuity_compare_followup':'Ampliación de contexto','continuity_model_followup':'Ampliación de modelo','continuity_correction_model':'Corrección con segundo modelo'}[scenario]
    cases = ('Detener tras observar 64 bytes pendientes de un relato ficticio solicitado de unas 400 palabras; reanudar el mismo hilo y responder de nuevo.'
             if scenario == 'stop_resume' else
             'Clave solo en una cita explícita; sesión nueva sin esa clave; Detener tras observar 64 bytes pendientes de un relato ficticio solicitado de unas 400 palabras; reanudar el mismo hilo.')
    if scenario == 'plugin_semantics':
        cases = 'Dos contactos llamados Cata: aclarar antes de ejecutar y conservar el texto exacto (dos turnos). Otra sesión recibe un resultado incierto por la ruta principal; observar si evita repetir por esa ruta o por una alternativa equivalente (un turno). Sin reintentos; un fallo detiene la etapa restante.'
    if scenario == 'continuity':
        cases = 'Dos hilos separados: consultar dos fuentes ficticias y entregar ambos valores; consultar una fuente ficticia que falla y continuar por una alternativa disponible. Un turno por caso, sin reintentos ni efectos externos. Un fallo de comportamiento se registra como resultado, no como fallo del protocolo.'
    if scenario == 'continuity_extended':
        cases = 'Dos hilos separados: consultar un trabajo ficticio que cambia a listo en la tercera consulta, con máximo de tres consultas; leer un resultado de delegación ficticia que solo completó A y comunica B pendiente. Un turno por caso, sin reintentos ni efectos externos. Una respuesta incompleta es una medición, no un fallo técnico.'
    if scenario == 'continuity_explicit':
        cases = 'Dos hilos separados: repetir el diseño de dos fuentes y espera, ahora nombrando explícitamente probe_read en la petición. Es un contraste exploratorio con muestras anteriores no simultáneas, no una repetición de esas muestras ni una demostración causal. Un turno por caso, hasta tres lecturas del estado, sin reintentos ni efectos externos.'
    if scenario == 'continuity_stall':
        cases = 'Dos hilos separados: una fuente siempre pendiente admite hasta seis lecturas ficticias y registra cada intento; una delegación ficticia devuelve A completa y B pendiente, nombrando probe_read. Un turno por caso, sin efectos externos ni repetición automática. La secuencia y la respuesta se revisan; el conteo no prueba por sí solo un bucle.'
    if scenario == 'continuity_correction':
        cases = 'Un hilo ficticio: consultar alpha y luego corregir el objetivo para consultar solo beta sin repetir alpha. Dos turnos, con una frontera explícita en la traza; sin efectos externos ni reintentos.'
    if scenario == 'continuity_compare':
        cases = 'Cuatro hilos: A1, B1, B2 y A2. Misma petición, modelo, herramienta, esquema y permisos; B agrega una frase factual en developerInstructions que identifica probe_read. Dos turnos por variante, uno por hilo, sin efectos externos ni reintentos. Dos muestras por variante son exploratorias, no una tasa precisa.'
    if scenario == 'continuity_model_compare':
        cases = 'Dos hilos con el mismo caso A y perfil ficticio de la comparación previa; cambia el modelo a gpt-6.1-sol/low. Dos muestras sin efectos externos ni reintentos. Las campañas no son simultáneas y no prueban causalidad ni tasas.'
    if scenario == 'continuity_compare_followup':
        cases = 'Cuatro muestras adicionales B3–A3–A4–B4 con el mismo contrato A/B: B añade una frase factual sobre la herramienta. Hilos separados, sin efectos externos ni reintentos.'
    if scenario == 'continuity_model_followup':
        cases = 'Dos muestras adicionales del caso A con gpt-6.1-sol/low, en hilos separados, sin efectos externos ni reintentos.'
    if scenario == 'continuity_correction_model':
        cases = 'Un hilo con gpt-6.1-sol/low: consultar alpha y luego corregir el objetivo para consultar solo beta sin repetir alpha. Dos turnos, datos ficticios, sin efectos externos ni reintentos.'
    text = f'''# Sonda de {title} — {run_id}

Estado: preparada, sin autorización live implícita.

- Escenario explícito: {scenario}, versión 2. Hasta {turns} turnos ChatGPT,
  modelo {plan['model']}, esfuerzo bajo; cero guía/OpenRouter.
- Casos secuenciales: {cases}
- Cada fase dispone de hasta 60 segundos, incluido su arranque; {turns} minutos
  de fases, más compilación previa y cierre. No se repite una muestra fallida o inconclusa.
- Solo datos ficticios y perfil/SQLite propios. Se enlaza la autenticación CLI
  existente únicamente en live; protocolo no enlaza cuenta ni inicia generación.
  El perfil/workspace queda en el temporal del sistema, fuera del checkout;
  informes y fuentes selladas quedan en el directorio de este plan.
- Sin MCP, archivos del usuario, comandos, control de apps, búsqueda web, plugins o multiagentes.
- Usa el adaptador Session/turn reales y SQLite del identificador de hilo;
  el diario de mensajes es un doble en memoria, NO Presence ni una prueba nueva de FRB/UI.
  La persistencia del fragmento en Presence mantiene su evidencia separada de D-047.
- El corte prueba cancelación local del consumidor y su hijo; no acredita ACK del
  servicio, cese de cómputo remoto o cese de facturación. Si ya terminó antes del corte,
  queda fallida/inconclusa sin repetirla automáticamente.
- No genera títulos auxiliares. Uso/tokens, reintentos internos y coste monetario
  efectivo son desconocidos si el adaptador no los observa; no se declaran cero.
- No toca la conversación ni la base personales, no aplica encargos o artefactos.

Protocolo sin cuenta: `python3 scripts/probes/chat_provider_probe.py protocol {directory}`.
Live requiere aprobación humana de este alcance y fuentes/binarios vigentes según
docs/rules/testing.md. El ACK evita un inicio accidental; no sustituye ese permiso.
Después de autorizar: `python3 scripts/probes/chat_provider_probe.py run {directory} --ack {run_id}`.

El protocolo prueba arranque, dos hilos distintos con notas locales ficticias,
lectura de sus datos materializados, rollouts separados, reapertura del mismo hilo
en un proceso nuevo e identificador activo
en SQLite, sin generación. No interpreta una lectura vacía/no disponible como
aislamiento. No prueba comprensión del modelo, cuotas, herramientas del bot, UI o Stop real.
Contrato de protocolo: https://learn.chatgpt.com/docs/app-server (verificado localmente
con el CLI sellado; la documentación no sustituye esa comprobación).
'''
    if scenario == 'plugin_semantics':
        text = text.replace('- Sin MCP, archivos del usuario, comandos, control de apps, búsqueda web, plugins o multiagentes.', '- MCP local por stdio/socket propio con cuatro herramientas ficticias; sin archivos del usuario, comandos, control de apps, búsqueda web, servicios de mensajería o multiagentes. Las funciones solo añaden efectos a un contador privado en memoria. Catálogo y ejecución tienen los esquemas públicos de Plugins; sus descripciones son abreviadas para el fixture, no se acredita el controlador real de Plugins.')
        start = text.index('- El corte prueba')
        end = text.index('- No genera títulos',start)
        text = text[:start] + '- Dos perfiles/hilos separados; contratos MCP comparados exactamente antes de generar. Cada intento cuenta aunque el fixture lo rechace; la ruta alternativa permite detectar duplicación sin efectos externos. No se puntúa la calidad por signos de interrogación ni por ausencia de efectos: respuestas conservadas para revisión humana. Un caso sin el envío principal esperado queda no verificado, no demuestra una conducta peligrosa.\n' + text[end:]
        start = text.index('El protocolo prueba')
        text = text[:start] + 'El protocolo abre ambas sesiones sin enlazar cuenta, descubre exactamente los cuatro esquemas y exige cero llamadas/efectos de las herramientas. No genera respuestas ni acredita comprensión del modelo, permisos de una integración real o calidad humana.\n'
    if scenario in CONTINUITY_CASES:
        text = text.replace('- Sin MCP, archivos del usuario, comandos, control de apps, búsqueda web, plugins o multiagentes.', '- MCP local con una herramienta ficticia de lectura; sin archivos del usuario, comandos, control de apps, búsqueda web, efectos externos ni multiagentes.')
        text = text.replace('- Solo datos ficticios y perfil/SQLite propios.', '- Solo datos ficticios y perfil/SQLite propios. El perfil se inicializa con las instrucciones predeterminadas de la migración vigente; no lee las configuraciones personales.')
        start = text.index('- El corte prueba')
        end = text.index('- No genera títulos',start)
        text = text[:start] + '- Cada caso usa hilo y perfil aislados; en corrección, ambos turnos comparten hilo. Se conservan llamadas, observaciones y respuesta ficticias para evaluación; no se atribuye éxito semántico al cierre técnico.\n' + text[end:]
        start = text.index('El protocolo prueba')
        text = text[:start] + 'El protocolo abre la sesión o sesiones sin cuenta, comprueba el esquema MCP exacto y exige cero llamadas y turnos de generación. No mide comportamiento del modelo, Presence, UI ni integraciones reales.\n'
    (directory/'PLAN.md').write_text(text)
    print(json.dumps({'directory':str(directory),'run_id':run_id,'scenario':scenario,'model_turns_max':turns,
                      'guide_calls_max':0,'live_started':False}))


def execute(directory, mode, ack=None):
    if mode not in ('protocol', 'live'):
        raise SystemExit('Modo de sonda no admitido.')
    directory = Path(directory).resolve()
    if (directory/'CLOSURE.json').exists():
        raise SystemExit('El plan está cerrado; no se vuelve a ejecutar.')
    plan = common.read(directory/'plan.json')
    seal = common.read(directory/'READY.json')
    if seal['plan_sha256'] != common.plan_digest(plan):
        raise SystemExit('El plan cambió; prepara uno nuevo.')
    validate_plan(plan,directory)
    if seal['sources'] != sources() or seal['binaries'] != binaries():
        raise SystemExit('Fuentes/binarios distintos; prepara otro plan.')
    if seal['archive_sha256'] != common.sha_file(directory/'sources.tar.gz'):
        raise SystemExit('El archivo de fuentes cambió; conserva el fallo y prepara otro plan.')
    if mode == 'live' and ack != seal['run_id']:
        raise SystemExit('Falta ACK exacto del plan autorizado.')
    invocation = dict(plan, mode=mode)
    target = directory/f'{mode}-invocation.json'
    common.save(target, invocation)
    env = os.environ.copy()
    for key in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY',
                'GENNIE_OPENROUTER_API_KEY','ACCESS_TOKEN','CHATGPT_ACCESS_TOKEN',
                'GENNIE_DATABASE_PATH'):
        env.pop(key, None)
    env['GENNIE_TEST_CHAT_PROVIDER_PLAN'] = str(target)
    with (directory/f'{mode}.log').open('x') as log:
        result = subprocess.run(['cargo','test','-p','gennie-llm','--lib','--locked',
                                 'chat_provider_probe','--','--ignored','--nocapture'],
                                cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    raw = directory/f'{mode}-result.json'
    try:
        report = common.read(raw)
        result_valid = report_matches_plan(report,plan,mode)
    except (OSError,ValueError):
        result_valid = False
    completion = {'exit_code':result.returncode,'source_drift':seal['sources'] != sources(),
                  'binary_drift':seal['binaries'] != binaries(),
                  'result_contract_passed':result_valid,
                  'result_sha256':common.sha_file(raw) if raw.is_file() else None}
    common.save(directory/f'{mode}-completion.json', completion)
    if result_valid and mode == 'live' and plan['scenario'] in CONTINUITY_CASES:
        cases = CONTINUITY_CASES[plan['scenario']]
        common.save(directory/'continuity-evaluation.json', {
            key:continuity_baseline.evaluate(case,report[key]['trace'])
            for key,case in cases.items()})
    print(json.dumps(completion))
    if result.returncode or not result_valid or completion['source_drift'] or completion['binary_drift']:
        raise SystemExit('Sonda fallida; evidencia conservada, sin reintento.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('prepare').add_argument('--scenario',choices=tuple(SCENARIOS),default='stop_resume')
    for command in ('protocol','run'):
        p = commands.add_parser(command)
        p.add_argument('directory')
        if command == 'run': p.add_argument('--ack', required=True)
    args = parser.parse_args()
    if args.command == 'prepare': prepare(args.scenario)
    else: execute(args.directory,'live' if args.command == 'run' else 'protocol',getattr(args,'ack',None))


if __name__ == '__main__': main()

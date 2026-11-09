#!/usr/bin/env python3
"""Adaptador de prueba para roles experimentales, con MCP ficticio y tokens reportados."""
import json
from pathlib import Path
import sys
import tempfile

import assignment_evaluation_worker as worker
import pilot_fixture
import pilot_mcp

SERVER = Path(__file__).with_name('pilot_mcp.py').resolve()


def usage_snapshot(value):
    """Toma solo campos del esquema local, sin sumar actualizaciones acumulativas."""
    allowed = ('inputTokens','cachedInputTokens','cacheWriteInputTokens','outputTokens','reasoningOutputTokens','totalTokens')
    if not isinstance(value,dict):
        return None
    groups = {}
    for group in ('last','total'):
        source = value.get(group)
        if isinstance(source,dict):
            counts = {key:source[key] for key in allowed if type(source.get(key)) is int and source[key]>=0}
            if counts:
                groups[group] = counts
    return groups or None


def parse_final(messages):
    """Interpreta el último mensaje completo; no concatena avisos previos."""
    if not isinstance(messages,list) or not messages or not isinstance(messages[-1],str):
        return None
    try:
        value = json.loads(messages[-1])
        return value if isinstance(value,dict) else None
    except (ValueError,TypeError):
        return None


def call(case, role, payload, *, mode='protocol', model='gpt-6.1-terra', effort='medium'):
    if case not in pilot_fixture.CASES or role not in ('architect','decision','planner','executor','diagnostician','stagnation_decision','parent_validator'):
        raise ValueError('invalid_scope')
    with tempfile.TemporaryDirectory(prefix='gennie-pilot-role-') as temporary:
        root = Path(temporary)
        trace = root/'calls.jsonl'
        config = ('mcp_servers.gennie={command='+json.dumps(sys.executable)+',args='+json.dumps([str(SERVER)])+
                  ',env={GENNIE_PILOT_CASE='+json.dumps(case)+',GENNIE_PILOT_TRACE='+json.dumps(str(trace))+'}}')
        instructions = (
            'Eres un rol experimental aislado del piloto. Los datos y herramientas son ficticios. '
            'No uses herramientas distintas de las registradas ni accedas a archivos o aplicaciones. '
            'Devuelve únicamente un objeto JSON válido, sin Markdown.\n'
            'Rol: '+role+'\n'
            'Contratos: architect={"objective":"..."}; decision={"engineer":"web"}; planner={"proposals":["...","..."]}; '
            'executor={"recipe":{"source":"main|backup","fields":{"nombre":{"selector":"tag.clase","attribute":"atributo"}}},"verdict":"..."}; '
            'diagnostician={"diagnosis":"..."}; stagnation_decision={"stalled":true|false}; parent_validator={"accepted":true|false}. '
            'El planner debe inspeccionar la página y proponer dos rutas. El executor debe inspeccionar y probar su receta. '
            'El validador debe comprobar el resultado con test_recipe. No inventes datos ausentes.'
        )
        spec = {'mode':mode,'base':str(root/'session'),'timeout_seconds':60,'model':model,'effort':effort,
                'mcp_config':config,'tool_names':[x['name'] for x in pilot_mcp.TOOLS],
                'expected_tool_schemas':{x['name']:x['inputSchema'] for x in pilot_mcp.TOOLS},
                'instructions':instructions,'request':json.dumps(payload,ensure_ascii=False)}
        original_snapshot = worker.usage_snapshot
        worker.usage_snapshot = usage_snapshot
        try:
            result = worker.run(spec)
        finally:
            worker.usage_snapshot = original_snapshot
        calls = [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []
        parsed = None
        if mode == 'live' and result['status'] == 'completed':
            parsed = parse_final(result.get('messages',[]))
            if parsed is None:
                result['status'] = 'invalid_json'
        sample = {'result':result,'response':parsed,'calls':calls}
    sample['temporary_removed'] = not root.exists()
    return sample

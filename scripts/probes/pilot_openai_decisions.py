#!/usr/bin/env python3
"""Cliente aislado para clasificaciones del piloto; no configura Gennie."""
import json
import os
from pathlib import Path
import shlex
import urllib.error
import urllib.request

ENDPOINT = 'https://api.openai.com/v1/decisions'
MODEL = 'gpt-6-luna'


class DecisionError(Exception):
    pass


def load_key(env_file):
    """El entorno tiene prioridad, incluso si está vacío; nunca exporta el secreto."""
    if 'OPENAI_API_KEY' in os.environ:
        return os.environ['OPENAI_API_KEY']
    if not env_file.exists():
        return None
    try:
        for line in env_file.read_text(encoding='utf-8').splitlines():
            line = line.strip()
            if line.startswith('export '):
                line = line[7:].lstrip()
            name, separator, value = line.partition('=')
            if separator and name.strip() == 'OPENAI_API_KEY':
                parts = shlex.split(value, comments=True)
                if len(parts) > 1:
                    raise DecisionError('invalid_openai_key_configuration')
                return parts[0] if parts else ''
    except (OSError, UnicodeError, ValueError):
        raise DecisionError('invalid_openai_key_configuration') from None
    return None


def choose(evidence, instructions, choices, *, api_key=None, opener=urllib.request.urlopen):
    """Una elección y su uso reportado; nunca registra la clave ni el cuerpo de error."""
    if not isinstance(evidence, str) or not isinstance(instructions, str) or not 2 <= len(choices) <= 20:
        raise DecisionError('invalid_request')
    if any(not isinstance(c, str) or not c.strip() for c in choices) or len(set(choices)) != len(choices):
        raise DecisionError('invalid_choices')
    key = api_key if api_key is not None else load_key(Path(__file__).resolve().parents[2] / '.env')
    if not key or not key.strip():
        raise DecisionError('missing_openai_api_key')
    payload = {'model': MODEL, 'input': evidence, 'questions': [
        {'type': 'choice', 'name': 'route', 'instructions': instructions,
         'choices': [{'value': choice} for choice in choices]}]}
    request = urllib.request.Request(ENDPOINT, data=json.dumps(payload).encode(), method='POST',
                                     headers={'Authorization': 'Bearer ' + key,
                                              'Content-Type': 'application/json'})
    try:
        with opener(request, timeout=20) as response:
            data = json.load(response)
    except (urllib.error.URLError, TimeoutError, OSError, ValueError):
        raise DecisionError('decisions_request_failed') from None
    if not isinstance(data, dict) or data.get('model') != MODEL:
        raise DecisionError('unexpected_response')
    answers = data.get('answers')
    if not isinstance(answers, list) or len(answers) != 1 or not isinstance(answers[0], dict):
        raise DecisionError('unexpected_response')
    answer = answers[0]
    if answer.get('type') != 'choice' or answer.get('name') != 'route' or answer.get('choice') not in choices:
        raise DecisionError('unexpected_answer')
    usage = data.get('usage')
    return {'choice': answer['choice'], 'usage': usage if isinstance(usage, dict) else None, 'model': MODEL}

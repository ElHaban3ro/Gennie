#!/usr/bin/env python3
"""Banco local y determinista: HTML ficticio, recetas declarativas y verificación ciega."""
from html.parser import HTMLParser
import re

CASES = {
    'simple': {
        'request':'Construye y verifica una receta que extraiga todos los enlaces mailto del directorio web ficticio en el campo emails.',
        'pages':{'main':'<main><a class="mail" href="mailto:ana@example.test">Ana</a><a class="mail" href="mailto:luis@example.test">Luis</a></main>'},
        'expected':{'emails':['mailto:ana@example.test','mailto:luis@example.test']},
    },
    'fallback': {
        'request':'Construye y verifica una receta que extraiga todos los enlaces mailto del directorio web ficticio en el campo emails.',
        'pages':{'backup':'<section><a class="contact" href="mailto:eva@example.test">Eva</a></section>'},
        'expected':{'emails':['mailto:eva@example.test']},
    },
    'compound': {
        'request':'Construye y verifica una receta que extraiga los enlaces mailto en el campo emails y los atributos datetime en el campo dates del directorio web ficticio.',
        'pages':{'main':'<main><a class="mail" href="mailto:ana@example.test">Ana</a><a class="mail" href="mailto:luis@example.test">Luis</a><time class="date" datetime="2026-11-01">Hoy</time><time class="date" datetime="2026-11-02">Mañana</time></main>'},
        'expected':{'emails':['mailto:ana@example.test','mailto:luis@example.test'],
                    'dates':['2026-11-01','2026-11-02']},
    },
}
SELECTOR = re.compile(r'([a-z][a-z0-9-]*)\.([a-z][a-z0-9_-]*)\Z')
ATTRIBUTE = re.compile(r'[a-z][a-z0-9_-]*\Z')


class Parser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.nodes = []

    def handle_starttag(self, tag, attrs):
        self.nodes.append((tag, dict(attrs)))


def inspect(case, source):
    if case not in CASES or source not in ('main','backup'):
        raise ValueError('invalid fixture request')
    if case == 'fallback' and source == 'main':
        return {'status':'unavailable','message':'La vía principal no responde; la interfaz alternativa backup está disponible.'}
    page = CASES[case]['pages'].get(source)
    return {'status':'ok','html':page} if page is not None else {'status':'not_found'}


def verify(case, recipe):
    """Ejecuta una receta limitada sobre HTML propio y compara sin revelar el esperado."""
    if case not in CASES or not isinstance(recipe, dict) or set(recipe) != {'source','fields'}:
        return {'status':'invalid_recipe','passed':False}
    source, fields = recipe['source'], recipe['fields']
    if source not in CASES[case]['pages'] or not isinstance(fields, dict) or not fields:
        return {'status':'invalid_recipe','passed':False}
    parser = Parser()
    parser.feed(CASES[case]['pages'][source])
    actual = {}
    for name, spec in fields.items():
        if not isinstance(name,str) or not isinstance(spec,dict) or set(spec) != {'selector','attribute'}:
            return {'status':'invalid_recipe','passed':False}
        selector, attr = spec['selector'], spec['attribute']
        if not isinstance(selector,str) or not isinstance(attr,str):
            return {'status':'invalid_recipe','passed':False}
        match = SELECTOR.fullmatch(selector)
        if not match or not ATTRIBUTE.fullmatch(attr):
            return {'status':'invalid_recipe','passed':False}
        tag, cls = match.groups()
        actual[name] = [attrs[attr] for node,attrs in parser.nodes
                        if node == tag and cls in attrs.get('class','').split() and attr in attrs]
    passed = actual == CASES[case]['expected']
    return {'status':'verified' if passed else 'mismatch','passed':passed,'actual':actual}

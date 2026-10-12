#!/usr/bin/env python3
"""Comprueba límites reales del workspace y rutas del índice documental."""
from pathlib import Path
import hashlib
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_PROMPT = '7412501ba941c357418096079a62cff4487dccf9cb138f197c36f4088ea3d9dd'
assert hashlib.sha256((ROOT/'.creation/initialization-prompt.md').read_bytes()).hexdigest() == EXPECTED_PROMPT, 'Prompt de origen alterado'
workspace = tomllib.loads((ROOT/'Cargo.toml').read_text())
for member in workspace['workspace']['members']:
    manifest = tomllib.loads((ROOT/member/'Cargo.toml').read_text())
    for dependency, specification in manifest.get('dependencies', {}).items():
        assert specification.get('workspace') is True, f'Dependencia no centralizada: {member}/{dependency}'
        if member.startswith('kernel/'):
            assert dependency not in ('soc-presence', 'soc'), f'Kernel depende de app: {member}'
for document in [ROOT/'AGENTS.md', ROOT/'README.md', *ROOT.glob('docs/**/*.md')]:
    for link in re.findall(r'\]\(([^)]+)\)', document.read_text()):
        if '://' in link or link.startswith('#'):
            continue
        assert (document.parent/link.split('#')[0]).exists(), f'Enlace ausente: {document}: {link}'
routes = {
    'frontend': ['frontend', 'modules', 'testing'],
    'backend': ['backend', 'testing'],
    'new_app': ['modules', 'backend', 'database', 'frontend', 'configuration', 'testing'],
}
for task, rules in routes.items():
    for name in rules:
        assert (ROOT/f'docs/rules/{name}.md').is_file(), f'Regla ausente para {task}: {name}'
print('OK: prompt íntegro, dependencias centrales, kernel independiente y enlaces/enrutamiento documental')

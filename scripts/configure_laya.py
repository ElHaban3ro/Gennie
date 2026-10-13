#!/usr/bin/env python3
"""Guarda rutas explícitas del runtime local, sin instalar ni descargar modelos."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--python', type=Path, required=True)
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--adapter', type=Path)
parser.add_argument('--output', type=Path)
args = parser.parse_args()
if args.output is None:
    args.output = Path.home()/'Library/Application Support/soc/decisions'/('runtime-v2.json' if args.adapter else 'runtime.json')
# Resolver el enlace del intérprete rompe la detección de pyvenv.cfg y pierde sus paquetes.
python, model = args.python.absolute(), args.model.resolve()
if not python.is_file() or not (model/'rl_agent_config.json').is_file() or not (model/'model.safetensors').is_file():
    parser.error('Faltan el intérprete o los archivos del checkpoint local de Laya')
if json.loads((model/'rl_agent_config.json').read_text()).get('encoder') != 'jhu-clsp/mmBERT-base':
    parser.error('Se requiere el checkpoint multilingüe de Laya, con encoder mmBERT-base')
args.output.parent.mkdir(parents=True, exist_ok=True)
config = {'python': str(python), 'model': str(model)}
if args.adapter:
    adapter = args.adapter.resolve()
    if not (adapter/'manifest.json').is_file():
        parser.error('Falta manifest.json de la adaptación')
    config['adapter'] = str(adapter)
args.output.write_text(json.dumps(config, indent=2)+'\n')
print(args.output)

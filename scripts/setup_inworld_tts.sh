#!/usr/bin/env bash
# Instala el transporte WebSocket en el entorno de voz existente.
set -euo pipefail
python_path="${HOME}/Library/Application Support/soc/tts/.venv/bin/python"
[[ -x "$python_path" ]] || { echo 'Falta el entorno de voz de Gennie.' >&2; exit 1; }
command -v uv >/dev/null || { echo 'Se necesita uv para preparar el entorno de voz.' >&2; exit 1; }
uv pip install --python "$python_path" --no-deps 'websockets==17.4'

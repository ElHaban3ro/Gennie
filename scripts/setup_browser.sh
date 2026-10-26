#!/usr/bin/env bash
# Prepara una instalación local versionada; nunca toca el perfil de navegación.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
browser_root="$HOME/Library/Application Support/Gennie/browser"
runtime="$browser_root/runtime-1.64.1"
command -v node >/dev/null || { echo 'Instala Node.js >=20 para preparar Playwright.' >&2; exit 1; }
node -e 'if(Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
umask 077
mkdir -p "$runtime" "$browser_root/browsers"
chmod 700 "$browser_root" "$runtime" "$browser_root/browsers"
cp "$root/apps/browser/runtime/package.json" "$root/apps/browser/runtime/package-lock.json" "$runtime/"
# Conservar la ubicación original de Node: Homebrew resuelve libnode junto a su binario.
ln -sfn "$(node -p 'process.execPath')" "$runtime/node"
npm ci --prefix "$runtime" --omit=dev --ignore-scripts
PLAYWRIGHT_BROWSERS_PATH="$browser_root/browsers" "$runtime/node" "$runtime/node_modules/playwright/cli.js" install chromium
browser_executable="$(PLAYWRIGHT_BROWSERS_PATH="$browser_root/browsers" "$runtime/node" -e 'console.log(require(process.argv[1]).chromium.executablePath())' "$runtime/node_modules/playwright")"
python3 "$root/scripts/brand_browser.py" "$runtime" "$browser_executable"
echo 'Playwright preparado. El perfil exclusivo se creará al abrir el navegador de Gennie.'

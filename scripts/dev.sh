#!/usr/bin/env bash
# Compila Rust y abre el escritorio Flutter con hot reload. Sin puertos de producto.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
if ! command -v flutter >/dev/null; then
  export PATH="${FLUTTER_ROOT:-$HOME/.local/share/gennie-flutter}/bin:$PATH"
fi
command -v flutter >/dev/null || { echo 'Instala Flutter estable o define FLUTTER_ROOT.' >&2; exit 1; }
if [[ "${1:-}" == '--generate' ]]; then
  command -v flutter_rust_bridge_codegen >/dev/null || export PATH="$HOME/.local/share/gennie-tools:$PATH"
  flutter_rust_bridge_codegen generate --config-file "$root/flutter_rust_bridge.yaml"
  shift
fi
if [[ $# -gt 0 && "${1:-}" != '--build-only' ]] || [[ $# -gt 1 ]]; then
  echo 'Uso: ./scripts/dev.sh [--generate] [--build-only]' >&2; exit 1
fi
[[ "$(uname -s)" == Darwin ]] || { echo 'El backend actual requiere macOS; Linux y Windows aún no están validados.' >&2; exit 1; }
if [[ "${1:-}" != '--build-only' ]]; then
  xcrun --find xcodebuild >/dev/null 2>&1 || { echo 'Falta Xcode completo. Instálalo y selecciona su directorio con xcode-select.' >&2; exit 1; }
fi
# Un único target para los dos lanzadores; no ejecutar ambos simultáneamente.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/flutter}"
mkdir -p "$CARGO_TARGET_DIR"
export CARGO_TARGET_DIR="$(cd "$CARGO_TARGET_DIR" && pwd)"
export MACOSX_DEPLOYMENT_TARGET=12.0
echo 'Compilando biblioteca Rust y helper local de Gennie…'
cargo build --locked -p gennie --lib --bin gennie
cd "$root/frontend"
flutter pub get
[[ "${1:-}" != '--build-only' ]] || exit 0
echo 'Abriendo Gennie en desarrollo. r: hot reload; R: reiniciar Dart; q: salir.'
echo 'Usa una base SQLite distinta si ya tienes otra instancia abierta.'
exec flutter run -d macos \
  --dart-define="GENNIE_BACKEND=$CARGO_TARGET_DIR/debug/libgennie_backend.dylib" \
  --dart-define="GENNIE_HELPER=$CARGO_TARGET_DIR/debug/gennie" \
  --dart-define="GENNIE_ENV=$root/.env"

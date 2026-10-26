#!/usr/bin/env bash
# Construye el paquete release; los runtimes de modelos y navegador se preparan por usuario.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
[[ $# -eq 0 ]] || { echo 'Uso: ./scripts/build.sh' >&2; exit 1; }
if ! command -v flutter >/dev/null; then
  export PATH="${FLUTTER_ROOT:-$HOME/.local/share/gennie-flutter}/bin:$PATH"
fi
command -v flutter >/dev/null || { echo 'Instala Flutter estable o define FLUTTER_ROOT.' >&2; exit 1; }
[[ "$(uname -s)" == Darwin ]] || { echo 'El backend actual requiere macOS; Linux y Windows aún no están validados.' >&2; exit 1; }
xcrun --find xcodebuild >/dev/null 2>&1 || { echo 'Falta Xcode completo. Instálalo y selecciona su directorio con xcode-select.' >&2; exit 1; }
# Rust y Flutter deben compilar para la misma arquitectura del host.
arch="$(uname -m)"
case "$arch" in arm64) rust_target=aarch64-apple-darwin ;; x86_64) rust_target=x86_64-apple-darwin ;; *) echo "Arquitectura no soportada: $arch" >&2; exit 1 ;; esac
export MACOSX_DEPLOYMENT_TARGET=12.0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/flutter}"
mkdir -p "$CARGO_TARGET_DIR"
export CARGO_TARGET_DIR="$(cd "$CARGO_TARGET_DIR" && pwd)"
echo 'Compilando Rust y Flutter en release…'
cargo build --release --target "$rust_target" --locked -p gennie --lib --bin gennie
cd "$root/frontend"
flutter pub get
flutter build macos --release --config-only
xcodebuild -workspace macos/Runner.xcworkspace -scheme Runner -configuration Release -derivedDataPath build/macos -arch "$arch" ONLY_ACTIVE_ARCH=YES CODE_SIGN_IDENTITY=- build
app="$root/frontend/build/macos/Build/Products/Release/Gennie.app"
mkdir -p "$app/Contents/Frameworks" "$app/Contents/Resources/ThirdParty/Inter"
cp "$CARGO_TARGET_DIR/$rust_target/release/libgennie_backend.dylib" "$app/Contents/Frameworks/"
cp "$CARGO_TARGET_DIR/$rust_target/release/gennie" "$app/Contents/MacOS/gennie-helper"
cp "$root/frontend/assets/fonts/LICENSE.txt" "$app/Contents/Resources/ThirdParty/Inter/"
install_name_tool -id '@rpath/libgennie_backend.dylib' "$app/Contents/Frameworks/libgennie_backend.dylib"
# Evita publicar un paquete que dependa de rutas del checkout o Homebrew.
for binary in "$app/Contents/Frameworks/libgennie_backend.dylib" "$app/Contents/MacOS/gennie-helper"; do
  while IFS= read -r dependency; do
    case "$dependency" in /System/Library/*|/usr/lib/*|@rpath/libgennie_backend.dylib) ;; *)
      echo "Dependencia no empaquetada: $dependency ($binary)" >&2; exit 1 ;;
    esac
  done < <(otool -L "$binary" | tail -n +2 | awk '{print $1}')
done
codesign --force --sign - "$app/Contents/Frameworks/libgennie_backend.dylib"
codesign --force --sign - "$app/Contents/MacOS/gennie-helper"
codesign --force --sign - --entitlements "$root/frontend/macos/Runner/Release.entitlements" "$app"
codesign --verify --deep --strict "$app"
echo "Aplicación creada: $app"
echo 'Firma local ad hoc; no incluye notarización ni instalación.'

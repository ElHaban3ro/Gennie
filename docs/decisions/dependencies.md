# Dependencias

## Navegador Patchright — 26 de octubre de 2026

`patchright` y `patchright-core` **1.64.2**, Apache-2.0, Node >=20. Versión, dependencias, engines e integridad comprobados en el [registro npm](https://registry.npmjs.org/patchright/1.64.2) y el [mantenedor](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright-nodejs). Lock npm versionado en `apps/browser/runtime`; preparación con `npm ci --ignore-scripts`. Patchright modifica el controlador compatible con Playwright y afecta solo Chromium; el mantenedor advierte diferencias como la consola desactivada y que no pasan todas las pruebas de Playwright. No garantiza evitar bloqueos.

`npm audit` en la preparación informó cero vulnerabilidades conocidas para los paquetes npm; no audita Chromium, Node ni páginas visitadas y no garantiza ausencia de fallos. Chromium se descarga con el instalador del mantenedor; diseño y límites en [D-026](browser.md).

**Pendiente de distribución:** Node.js >=20 y Chromium son dependencias externas. La aplicación release no los incluye; cada usuario necesita preparar el runtime, y el enlace a Node depende de que su instalación permanezca en la misma ruta. Una distribución autónoma con Node privado y Chromium incluido o descargado de forma guiada queda pendiente; no se implementa en este cambio. Python 3 y las herramientas macOS también son necesarias para preparar el bundle de Gennie Web.

Actualización de interfaz del 24 de octubre de 2026: Flutter 3.47.8 con Dart 3.13.7; puente flutter_rust_bridge 2.13.0, licencia MIT. La biblioteca Rust y el paquete Dart usan la misma versión del puente. Rust mínimo del workspace: 1.92. Lockfiles versionados; no equivalen a certificación de seguridad.

| Dependencia UI | Versión | Propósito / fuente |
|---|---|---|
| Flutter / Dart | 3.47.8 / 3.13.7 | Escritorio; [archivo oficial](https://docs.flutter.dev/install/archive), licencias BSD |
| flutter_rust_bridge | 2.13.0 | FFI generado Rust/Dart, MIT; [registro Rust](https://crates.io/crates/flutter_rust_bridge/2.13.0), [registro Dart](https://pub.dev/packages/flutter_rust_bridge/versions/2.13.0) |
| flutter_svg | 2.3.0 | Assets SVG; [registro](https://pub.dev/packages/flutter_svg/versions/2.3.0) |
| flutter_markdown_plus | 1.0.12 | Respuestas Markdown; [registro](https://pub.dev/packages/flutter_markdown_plus/versions/1.0.12). Sustituye flutter_markdown descontinuado |
| url_launcher | 6.3.3 | Enlaces por acción del usuario; [registro](https://pub.dev/packages/url_launcher/versions/6.3.3) |
| window_manager | 0.5.2 | Ciclo de ventana; [registro](https://pub.dev/packages/window_manager/versions/0.5.2) |
| Inter | 4.1 | TTF locales 400/500/600/700, SIL OFL 1.1; [origen](https://github.com/rsms/inter/releases/tag/v4.1) |

Conservar licencias en assets y paquete. La revisión del backend siguiente conserva contexto de su fecha original; revisar el lockfile actual antes de concluir sobre dependencias transitivas o avisos. La migración elimina dependencias del renderer anterior y sus adaptadores; la ventana pertenece al runner Flutter.

| Dependencia backend | Exacta | Features/motivo | Fuentes |
|---|---|---|---|
| rusqlite | 0.40.2 | defaults off; bundled evita SQLite distinto por OS; sin extensiones/crypto | [registro](https://crates.io/crates/rusqlite/0.40.2), [mantenedor](https://github.com/rusqlite/rusqlite) |
| dotenvy | 0.15.7 | parser .env sin mutar entorno; MSRV 1.56.1 | [registro](https://crates.io/crates/dotenvy/0.15.7), [mantenedor](https://github.com/allan2/dotenvy) |
| sha2 | 0.11.0 | defaults off; checksum de migraciones; MSRV 1.85 | [registro](https://crates.io/crates/sha2/0.11.0), [mantenedor](https://github.com/RustCrypto/hashes) |


## Avisos y límites
Revisión histórica del 11 de octubre; no describe el lockfile posterior a Flutter. Consultada [RustSec advisory-db](https://github.com/RustSec/advisory-db) actual: rusqlite RUSTSEC-2020-0014 y 2021-0128 están corregidos en la versión elegida; sha2 RUSTSEC-2021-0100 afecta 0.9.7, no 0.11.0. No directorio encontrado para dotenvy en ese snapshot; esto no prueba ausencia de vulnerabilidad ni compromiso.

## Dictado local — 2026-10-11
Versiones exactas comprobadas con APIs oficiales crates.io y fuentes de mantenedores descargadas antes de incorporar:

| Dependencia | Versión | Motivo/MSRV/licencia | Fuente |
|---|---|---|---|
| transcribe-cpp | 0.3.1 | Motor usado por Handy 0.9.8; Metal, defaults off; Rust 1.74, MIT. sys 0.3.1 contiene C++/ggml | [registro](https://crates.io/crates/transcribe-cpp/0.3.1), [mantenedor](https://github.com/handy-computer/transcribe.cpp) |
| cpal | 0.16.0 | CoreAudio, defaults off; Rust 1.70, Apache-2.0 | [registro](https://crates.io/crates/cpal/0.16.0), [mantenedor](https://github.com/RustAudio/cpal) |
| handy-keys | 0.3.4 | Atajo global pressed/released y bloqueo; MIT, registro no declara MSRV; compilado con workspace 1.95 | [registro](https://crates.io/crates/handy-keys/0.3.4), [mantenedor](https://github.com/handy-computer/handy-keys) |
| reqwest | 0.12.28 | Descarga HTTPS aislada, blocking/rustls-tls, defaults off; Rust 1.64, MIT OR Apache-2.0 | [registro](https://crates.io/crates/reqwest/0.12.28) |
| serde / serde_json | 1.0.229 / 1.0.154 | IPC de estados y texto efímero, derive; Rust 1.56 / 1.71, MIT OR Apache-2.0 | [serde](https://crates.io/crates/serde/1.0.229), [serde_json](https://crates.io/crates/serde_json/1.0.154) |

No Tauri, ONNX, CUDA ni servidores en Gennie. Comprobados manifests/features del artefacto macOS; transcribe-cpp 0.3.1 coincide con la integración Handy inspeccionada, sin asumir que la versión más reciente 0.4.0 preserve su API. CMake y compilador C++ son necesarios para construir el motor. Modelo Q8_0 con licencia OpenMDW-1.1: [ficha oficial NVIDIA](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) y [conversión Handy](https://huggingface.co/handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/tree/8139c4ec14bdc45c361adf8d57c27c28e7478272). SHA-256 LFS b94545b313b3223fda7b2857a52681da813935c2127643d1e9ff0c23d988089c; 751094240 bytes. No costo por minuto local; no prueba de ser mejor modelo en todas las condiciones.

RustSec revisado en snapshot 7eebec69c352c7191b1f13eb95dd510eeca5d1de: sin directorios para nuevas dependencias directas CPAL/handy-keys/transcribe-cpp/reqwest. Avisos de red del lock comparados por rangos: rustls 0.23.45 corrige RUSTSEC-2026-0285; webpki 0.103.15 cubre hasta RUSTSEC-2026-0104; bytes 1.12.1 corrige 2026-0007; quinn-proto 0.11.19 cubre 2026-0185; tokio 1.53.2, hyper 1.12.0 y demás versiones revisadas están en rangos corregidos. Ring 0.17.14 tiene aviso informativo unmaintained 2025-0007: se conserva como proveedor TLS transitivo de reqwest y exige reevaluación, junto con los límites históricos de esa revisión. No equivale a cargo audit, auditoría de C++/ggml o garantía de ausencia de vulnerabilidades.

## Runtime externo Laya — 2026-10-12
Sin dependencias Rust nuevas de registro: gennie-decisions reutiliza core/db/serde/serde_json y se añade al lock como crate de workspace. Se reutiliza el entorno instalado de PromptInspector, sin modificar sus paquetes: Python 3.12.13, laya 0.3.5, torch 2.14.0, transformers 5.17.0, safetensors 0.8.0. [Registro exacto Laya](https://pypi.org/project/laya/0.3.5/) y [modelo del mantenedor](https://huggingface.co/convaiinnovations/laya). Revisión multilingual 7b928d828b7b0e022f929d9bd2e44165aa270148 comprobada en el checkpoint local de la comparación; configuración max_len=1024, head_max_len=256. La inferencia está forzada offline y las rutas son explícitas. No se incorporó una instalación transitiva nueva ni una auditoría completa de Python/PyTorch; la reutilización no certifica ausencia de vulnerabilidades. No usar afirmaciones de calibración del mantenedor como garantía del ejecutor: las pruebas locales mostraron selecciones incorrectas.

## Voz Kokoro MLX — 2026-10-13
`gennie-tts` reutiliza core/db/serde/serde_json sin nuevos crates externos. Entorno aislado en datos de Gennie: Python 3.12.13, `kokoro-mlx` 0.1.2 y `sounddevice` 0.5.6, instalados y probados en Apple Silicon. El [mantenedor](https://github.com/gabrimatic/kokoro-mlx) declara macOS 13+, Python 3.10–3.12, MLX 0.31+, inferencia local, voz española y licencia MIT para el código; los pesos Kokoro se descargan aparte con licencia Apache 2.0. La instalación transitiva de Python no equivale a auditoría de sus 93 paquetes ni fija el hash de los pesos. El modelo descargado en este Mac produjo WAV y audio por altavoces; no demuestra todos los dispositivos de salida ni una instancia UI nueva.

Para preparar las respuestas con citas, `gennie-presence` usa `pulldown-cmark` =0.13.4 ya presente en Cargo.lock, ahora como dependencia directa con `default-features = false`; no se incorporan sus features HTML/CLI. El manifiesto publicado indica licencia MIT y Rust 1.71.1 mínimo. Su [API oficial](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/) expone eventos de Markdown para separar texto y enlaces sin renderizar HTML.

## Reminders plugin — 2026-10-14

`gennie-reminders-plugin` es un crate local: reutiliza `gennie-core`, `gennie-config` en pruebas y las versiones ya fijadas de `serde`/`serde_json`. No incorpora versiones nuevas de registro ni paquetes Swift externos. El puente compila con Foundation y EventKit del SDK de macOS instalado; requiere `swiftc` durante el build. Esta incorporación no equivale a una auditoría nueva de las dependencias preexistentes.

## Compilación con Xcode 27 — 24 de octubre de 2026

Con Xcode 27.1, Rust 1.95 y destino macOS 12, la biblioteca de `zerofrom-derive` generada en release no cargó: `mis-aligned LINKEDIT string pool`. La firma era válida; `ctypes.CDLL` reprodujo el rechazo y `cargo build --release --target aarch64-apple-darwin --locked -p zerofrom` falló con E0463. El mismo comando con `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=none` pasó. Se fija `profile.release.build-override.strip=false` en el workspace para conservar símbolos en proc-macros/build scripts, sin cambiar versiones ni optimización del producto. Coincide con los defectos documentados de [Rust](https://github.com/rust-lang/rust/issues/157750) y [LLVM](https://github.com/llvm/llvm-project/issues/203678); reevaluar el ajuste cuando cambie el toolchain.

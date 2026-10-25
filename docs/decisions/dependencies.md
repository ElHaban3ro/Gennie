# Dependencias — revisión 2026-10-11

Elección delegada para esta construcción. Comprobado con API oficial crates.io y repositorios de mantenedores antes de incorporar. Rust local 1.95.0; mínimo workspace 1.92 por Slint. Cargo.lock se conserva para reproducibilidad, no es certificación de seguridad.

| Dependencia directa | Exacta | Features/motivo | Fuentes |
|---|---|---|---|
| Slint | 1.18.1 | defaults off; std, backend-winit, renderer-femtovg, compat-1-18, accessibility, unstable-winit-030. UI nativa, spring, políticas macOS | [crates.io](https://crates.io/crates/slint/1.18.1), [release](https://github.com/slint-ui/slint/releases/tag/v1.18.1), [MSRV/features](https://docs.rs/crate/slint/1.18.1), [spring](https://docs.slint.dev/latest/docs/slint/reference/language/animations/) |
| slint-build | 1.18.1 | build only, compila archivo UI separado | [registro](https://crates.io/crates/slint-build/1.18.1), mismo mantenedor/release |
| rusqlite | 0.40.2 | defaults off; bundled evita SQLite distinto por OS; sin extensiones/crypto | [registro](https://crates.io/crates/rusqlite/0.40.2), [mantenedor](https://github.com/rusqlite/rusqlite) |
| dotenvy | 0.15.7 | parser .env sin mutar entorno; MSRV 1.56.1 | [registro](https://crates.io/crates/dotenvy/0.15.7), [mantenedor](https://github.com/allan2/dotenvy) |
| objc2-app-kit | 0.3.2 | solo macOS; defaults off, std/NSEvent/NSView/NSWindow; cursor para hit test y política de Spaces sobre la ventana real; MSRV 1.71 | [registro](https://crates.io/crates/objc2-app-kit/0.3.2), [mantenedor](https://github.com/madsmtm/objc2) |
| sha2 | 0.11.0 | defaults off; checksum de migraciones; MSRV 1.85 | [registro](https://crates.io/crates/sha2/0.11.0), [mantenedor](https://github.com/RustCrypto/hashes) |

Winit 0.30.13 resuelto por Slint y usado mediante reexport; no segunda versión directa. API unstable se aísla y exige revisar al subir Slint. Transparencia y always-on-top son capacidades de plataforma, no garantías universales.

## Licencia
Slint permite elección Royalty-free para desktop, GPLv3 o comercial. Para esta base local se incorpora AboutSlint accesible desde el menú nativo gennie → Acerca de gennie. No se ha decidido licencia de distribución del proyecto ni se ha publicado un binario; la entrega Git posterior está registrada por separado. Revisar condiciones exactas al distribuir o incorporar embedded: [licencia versionada](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md), [texto royalty-free](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).

## Avisos y límites
Consultada [RustSec advisory-db](https://github.com/RustSec/advisory-db) actual: rusqlite RUSTSEC-2020-0014 y 2021-0128 están corregidos en la versión elegida; sha2 RUSTSEC-2021-0100 afecta 0.9.7, no 0.11.0. No directorio encontrado para slint/slint-build/dotenvy en ese snapshot; esto no prueba ausencia de vulnerabilidad ni compromiso.

Se extrajeron avisos correspondientes a los paquetes del lockfile completo y se compararon rangos manualmente. Las vulnerabilidades listadas están cubiertas por rangos corregidos/no afectados de las versiones resueltas, incluyendo arrayref 0.3.9 (unaffected <=0.3.9). Informativos de falta de mantenimiento: bincode 2.0.1, ttf-parser 0.25.1 y xml-rs 0.8.29. Cargo metadata filtrado para aarch64-apple-darwin confirma bincode en el grafo efectivo (typed-index-collections 3.5.0 lo introduce); ttf-parser y xml-rs no aparecen en ese grafo. Bincode se mantiene transitivo en Slint; aviso de mantenimiento, no vulnerabilidad demostrada. Reevaluar al actualizar. No equivalencia a cargo audit (no instalado), análisis de supply chain ni auditoría exhaustiva. Crates.io verifica descargas mediante checksums; revisar avisos nuevamente al actualizar.

NSEvent::mouseLocation leído en main cada 16 ms; no instala event taps ni requiere acceso a contenido de otras apps. RustSec no tenía directorio objc2-app-kit/objc2 en el snapshot consultado; no certifica ausencia de avisos. Hit test macOS dinámico: ventana ignora mouse fuera del círculo; latencia depende del event loop. Otras plataformas aún no implementan click-through.

## Extensión AppKit para Spaces — 2026-10-11
Se verificaron features NSView y NSWindow en crates.io 0.3.2 y bindings del paquete oficial (MSRV 1.71), sin nueva dependencia ni actualización de versión. Winit 0.30.13 reexporta raw-window-handle 0.6; el handle AppKit presta el NSView de la ventana viva. Acceso limitado al hilo principal, sin retener el puntero. Política con APIs AppKit documentadas: [canJoinAllSpaces](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/canjoinallspaces) y [fullScreenAuxiliary](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/fullscreenauxiliary). Avisos del paquete no se reinterpretan como certificación.

## Icono de barra superior — 2026-10-11
Se incorpora tray-icon =0.26.1 solo macOS, defaults off, por petición explícita del usuario. MSRV 1.90, MIT OR Apache-2.0; verificados en la [API de crates.io](https://crates.io/api/v1/crates/tray-icon/0.26.1) y [manifest del mantenedor](https://github.com/tauri-apps/tray-icon/blob/tray-icon-v0.26.1/Cargo.toml). No GTK, AppIndicator ni KSNI. Usa AppKit y muda 0.21.2 (MSRV 1.90, Apache-2.0 OR MIT); Slint conserva su muda 0.19.3, con receptores de eventos separados. Lockfile añade tray-icon, muda 0.21.2 y keyboard-types 0.8.3.

El [mantenedor](https://github.com/tauri-apps/tray-icon) exige crear el icono en main cuando el event loop ya esté ejecutándose: se hace tras winit_window().await y se conserva hasta finalizar la UI. SVG propio monocromático embebido, rasterizado por Slint existente, marcado template para apariencia macOS. Eventos de menú procesados en el timer existente del event loop (16 ms), sin worker adicional. Destrucción RAII retira NSStatusItem incluso ante errores.

Consultados los directorios tray-icon y muda de [RustSec advisory-db](https://github.com/RustSec/advisory-db): API respondió 404 para ambos. No equivale a auditoría de vulnerabilidades ni revisión exhaustiva de tránsito. Se mantiene la limitación previa de bincode.

## Dictado local — 2026-10-11
Versiones exactas comprobadas con APIs oficiales crates.io y fuentes de mantenedores descargadas antes de incorporar:

| Dependencia | Versión | Motivo/MSRV/licencia | Fuente |
|---|---|---|---|
| transcribe-cpp | 0.3.1 | Motor usado por Handy 0.9.8; Metal, defaults off; Rust 1.74, MIT. sys 0.3.1 contiene C++/ggml | [registro](https://crates.io/crates/transcribe-cpp/0.3.1), [mantenedor](https://github.com/handy-computer/transcribe.cpp) |
| cpal | 0.16.0 | CoreAudio, defaults off; Rust 1.70, Apache-2.0 | [registro](https://crates.io/crates/cpal/0.16.0), [mantenedor](https://github.com/RustAudio/cpal) |
| handy-keys | 0.3.4 | Atajo global pressed/released y bloqueo; MIT, registro no declara MSRV; compilado con workspace 1.95 | [registro](https://crates.io/crates/handy-keys/0.3.4), [mantenedor](https://github.com/handy-computer/handy-keys) |
| reqwest | 0.12.28 | Descarga HTTPS aislada, blocking/rustls-tls, defaults off; Rust 1.64, MIT OR Apache-2.0 | [registro](https://crates.io/crates/reqwest/0.12.28) |
| serde / serde_json | 1.0.229 / 1.0.154 | IPC de estados y texto efímero, derive; Rust 1.56 / 1.71, MIT OR Apache-2.0 | [serde](https://crates.io/crates/serde/1.0.229), [serde_json](https://crates.io/crates/serde_json/1.0.154) |

No Tauri, ONNX, CUDA ni servidores en gennie. Comprobados manifests/features del artefacto macOS; transcribe-cpp 0.3.1 coincide con la integración Handy inspeccionada, sin asumir que la versión más reciente 0.4.0 preserve su API. CMake y compilador C++ son necesarios para construir el motor. Modelo Q8_0 con licencia OpenMDW-1.1: [ficha oficial NVIDIA](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) y [conversión Handy](https://huggingface.co/handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/tree/8139c4ec14bdc45c361adf8d57c27c28e7478272). SHA-256 LFS b94545b313b3223fda7b2857a52681da813935c2127643d1e9ff0c23d988089c; 751094240 bytes. No costo por minuto local; no prueba de ser mejor modelo en todas las condiciones.

RustSec revisado en snapshot 7eebec69c352c7191b1f13eb95dd510eeca5d1de: sin directorios para nuevas dependencias directas CPAL/handy-keys/transcribe-cpp/reqwest. Avisos de red del lock comparados por rangos: rustls 0.23.45 corrige RUSTSEC-2026-0285; webpki 0.103.15 cubre hasta RUSTSEC-2026-0104; bytes 1.12.1 corrige 2026-0007; quinn-proto 0.11.19 cubre 2026-0185; tokio 1.53.2, hyper 1.12.0 y demás versiones revisadas están en rangos corregidos. Ring 0.17.14 tiene aviso informativo unmaintained 2025-0007: se conserva como proveedor TLS transitivo de reqwest y exige reevaluación, junto con límites previos de Slint. No equivale a cargo audit, auditoría de C++/ggml o garantía de ausencia de vulnerabilidades.

Menú nativo de transcripción: i-slint-backend-winit =1.18.1 se declara directamente (la misma versión ya resuelta), defaults off, renderer-femtovg/accessibility. Registro oficial confirma Rust 1.92 y las mismas alternativas de licencia de Slint. Permite desactivar with_default_menu_bar: Slint sobrescribía el menú propio cuando activaba Settings/About sin MenuBar. Se retira MenuBar declarativo de presencia en macOS y status_menu posee ambos menús, eventos y Cmd+Q con cierre normal; sin reinstalar menús en cada frame. Fuente: [registro](https://crates.io/crates/i-slint-backend-winit/1.18.1), [implementación del mantenedor](https://github.com/slint-ui/slint/tree/v1.18.1/internal/backends/winit). No segunda versión ni segundo event loop.

## Límites del marco flotante — 2026-10-11
Acceso directo macOS a objc2 =0.6.5 (std) y objc2-foundation =0.3.2 (std/NSGeometry), ya presentes en el lockfile: sin versiones ni paquetes adicionales. Verificados en API oficial [objc2](https://crates.io/api/v1/crates/objc2/0.6.5) y [foundation](https://crates.io/api/v1/crates/objc2-foundation/0.3.2): MIT, MSRV 1.71. RustSec devolvió 404 para los directorios de ambos paquetes; no es auditoría de seguridad.

La política se añade como método de la clase Winit del proceso, sin reemplazar NSWindow ni modificar isa/layout/ivars. La firma y el encoding son los de constrainFrameRect:toScreen: de NSWindow. Se valida el ancestro directo y que Winit no tenga implementación propia; al actualizar Winit reevaluar. Solo la identidad de la ventana propia omite el límite; cualquier otra instancia delega a NSWindow. La identidad no se dereferencia ni retiene NSView; la UI mantiene viva la ventana y la retira antes de ocultarla al salir. Sonda nativa comprueba aislamiento y restauración.

## Runtime externo Laya — 2026-10-12
Sin dependencias Rust nuevas de registro: gennie-decisions reutiliza core/db/serde/serde_json y se añade al lock como crate de workspace. Se reutiliza el entorno instalado de PromptInspector, sin modificar sus paquetes: Python 3.12.13, laya 0.3.5, torch 2.14.0, transformers 5.17.0, safetensors 0.8.0. [Registro exacto Laya](https://pypi.org/project/laya/0.3.5/) y [modelo del mantenedor](https://huggingface.co/convaiinnovations/laya). Revisión multilingual 7b928d828b7b0e022f929d9bd2e44165aa270148 comprobada en el checkpoint local de la comparación; configuración max_len=1024, head_max_len=256. La inferencia está forzada offline y las rutas son explícitas. No se incorporó una instalación transitiva nueva ni una auditoría completa de Python/PyTorch; la reutilización no certifica ausencia de vulnerabilidades. No usar afirmaciones de calibración del mantenedor como garantía del ejecutor: las pruebas locales mostraron selecciones incorrectas.

## Voz Kokoro MLX — 2026-10-13
`gennie-tts` reutiliza core/db/serde/serde_json sin nuevos crates externos. Entorno aislado en datos de Gennie: Python 3.12.13, `kokoro-mlx` 0.1.2 y `sounddevice` 0.5.6, instalados y probados en Apple Silicon. El [mantenedor](https://github.com/gabrimatic/kokoro-mlx) declara macOS 13+, Python 3.10–3.12, MLX 0.31+, inferencia local, voz española y licencia MIT para el código; los pesos Kokoro se descargan aparte con licencia Apache 2.0. La instalación transitiva de Python no equivale a auditoría de sus 93 paquetes ni fija el hash de los pesos. El modelo descargado en este Mac produjo WAV y audio por altavoces; no demuestra todos los dispositivos de salida ni una instancia UI nueva.

Para preparar las respuestas con citas, `gennie-presence` usa `pulldown-cmark` =0.13.4 ya presente en Cargo.lock, ahora como dependencia directa con `default-features = false`; no se incorporan sus features HTML/CLI. El manifiesto publicado indica licencia MIT y Rust 1.71.1 mínimo. Su [API oficial](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/) expone eventos de Markdown para separar texto y enlaces sin renderizar HTML.

La sonda visual aislada usa la feature de desarrollo `renderer-software` de Slint 1.18.1. Cargo.lock incorpora `i-slint-backend-testing` 1.18.1 para esa feature; no cambia el renderer de producción Winit/FemtoVG. La sonda renderiza fuera del escritorio y verifica el callback de un clic sintético, sin afirmar apertura física del navegador.

## Reminders plugin — 2026-10-14

`gennie-reminders-plugin` es un crate local: reutiliza `gennie-core`, `gennie-config` en pruebas y las versiones ya fijadas de `serde`/`serde_json`. No incorpora versiones nuevas de registro ni paquetes Swift externos. El puente compila con Foundation y EventKit del SDK de macOS instalado; requiere `swiftc` durante el build. Esta incorporación no equivale a una auditoría nueva de las dependencias preexistentes.

## Compilación con Xcode 27 — 24 de octubre de 2026

Con Xcode 27.1, Rust 1.95 y destino macOS 12, la biblioteca de `zerofrom-derive` generada en release no cargó: `mis-aligned LINKEDIT string pool`. La firma era válida; `ctypes.CDLL` reprodujo el rechazo y `cargo build --release --target aarch64-apple-darwin --locked -p zerofrom` falló con E0463. El mismo comando con `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=none` pasó. Se fija `profile.release.build-override.strip=false` en el workspace para conservar símbolos en proc-macros/build scripts, sin cambiar versiones ni optimización del producto. Coincide con los defectos documentados de [Rust](https://github.com/rust-lang/rust/issues/157750) y [LLVM](https://github.com/llvm/llvm-project/issues/203678); reevaluar el ajuste cuando cambie el toolchain.

# Escritorio Flutter de Gennie

`lib` contiene la interfaz y `lib/src/rust` el puente generado por flutter_rust_bridge. Rust conserva reglas, persistencia, modelos y procesos. El frontend no abre SQLite ni implementa otro backend.

Desde la raíz del repositorio:

```sh
./scripts/dev.sh
./scripts/dev.sh --build-only
./scripts/dev.sh --generate --build-only
./scripts/build.sh
```

El primer comando abre macOS en debug con hot reload de Dart. El segundo compila Rust y resuelve paquetes sin abrir la aplicación ni compilar el runner. El tercero regenera la API con codegen 2.13.0. El último construye `build/macos/Build/Products/Release/Gennie.app`, sin instalarla ni abrirla. No ejecutar los lanzadores a la vez: comparten `target/flutter`.

Flutter 3.47.8 / Dart 3.13.7; flutter_rust_bridge 2.13.0. Inter 4.1 se distribuye como assets locales, pesos 400/500/600/700 y licencia OFL. SVG, Markdown, enlaces y ventana usan flutter_svg, flutter_markdown_plus, url_launcher y window_manager. Versiones en `pubspec.yaml`; resolución en `pubspec.lock`.

El proceso Flutter carga `libgennie_backend.dylib`; el helper Rust `gennie` separado contiene audio/inferencia. En desarrollo, los dart-defines `GENNIE_BACKEND`, `GENNIE_HELPER` y `GENNIE_ENV` los entrega el lanzador. En release, el paquete incluye biblioteca y helper; sin configuración externa se usa explícitamente `~/Library/Application Support/Gennie/gennie.sqlite3`. Desarrollo conserva `.env`; no se copia su DB al paquete. Cerrar ventana la oculta; Salir solicita cierre Rust asíncrono mediante `gennie/lifecycle` (`requestQuit`). No usar `Platform.resolvedExecutable` como helper Rust.

macOS requiere Xcode completo. El paquete release se compiló y firmó con Xcode 27.1; las pruebas Dart y Rust no sustituyen la verificación visual ni la interacción nativa. El producto no abre HTTP ni listeners; el modo debug de Flutter puede usar servicios locales temporales. No replicar cara, presencia flotante ni globos.

Reglas y diseño vigentes: [frontend](../docs/rules/frontend.md) y [UI](../UI.md).

Los lanzadores admiten `CARGO_TARGET_DIR` para aislar la caché Rust; el valor predeterminado es `target/flutter`. Por ejemplo, `CARGO_TARGET_DIR=/tmp/gennie-flutter-target ./scripts/dev.sh --build-only`. Conservar el mismo directorio al compilar biblioteca y helper; evitar procesos Cargo simultáneos sobre él.

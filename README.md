# Gennie

Asistente de escritorio local para macOS con frontend Flutter y backend Rust mediante flutter_rust_bridge. Conserva SQLite, historial propio por conversación, dictado Nemotron, conversación mediante la sesión ChatGPT de Codex CLI y narración Kokoro. Laya y Recordatorios permanecen como módulos locales independientes. La presencia flotante, la cara y los globos se retiraron.

## Desarrollo

Requiere Rust 1.92 o superior, Flutter 3.47.8 (Dart 3.13.7) y Xcode completo para compilar y ejecutar macOS. Las Command Line Tools por sí solas no bastan. Instala Flutter en PATH o define `FLUTTER_ROOT`; el lanzador también busca `~/.local/share/gennie-flutter`.

```sh
./scripts/dev.sh
```

Compila la biblioteca Rust y el helper en `target/flutter/debug`, resuelve dependencias Flutter y abre Gennie en debug. `r` recarga Dart, `R` reinicia Dart y `q` sale. Cambiar Rust requiere volver a ejecutar el lanzador. `./scripts/dev.sh --build-only` prepara Rust y dependencias sin abrir UI ni DB; no compila el runner macOS. `./scripts/dev.sh --generate --build-only` regenera antes el puente con `flutter_rust_bridge_codegen` 2.13.0 instalado. No ejecutar dos procesos Cargo sobre `target/flutter`.

El frontend está en [frontend](frontend/README.md); el puente se configura en [flutter_rust_bridge.yaml](flutter_rust_bridge.yaml). `libgennie_backend.dylib` corre dentro del proceso Flutter; el ejecutable `gennie` separado permite los procesos hijos de audio/inferencia. El binario Rust solo no es un lanzador de escritorio. El producto no expone HTTP ni puertos; Flutter puede abrir servicios temporales de depuración.

Crea `.env` desde [.env.example](.env.example) solo en instalaciones nuevas; conserva la ruta de una base existente. En desarrollo, `GENNIE_DATABASE_PATH` es obligatoria en el entorno o `.env`. El escritorio resuelve rutas relativas respecto al directorio del `.env` explícito; las herramientas CLI mantienen el cwd. Usa una ruta absoluta al elegir una base existente para el paquete. No abrir otra instancia sobre la misma DB. Para conversar instala Codex CLI e inicia sesión mediante `codex login`; Gennie no almacena sus tokens.

## Compilar la aplicación

```sh
./scripts/build.sh
```

Crea `frontend/build/macos/Build/Products/Release/Gennie.app` con la dylib, el helper y la licencia Inter. Firma local ad hoc; no instala, abre ni notariza la aplicación. Sin `GENNIE_ENV` ni `GENNIE_DATABASE_PATH`, el paquete release usa explícitamente `~/Library/Application Support/Gennie/gennie.sqlite3`; no copia automáticamente los datos del checkout. Para usar una base existente, indica su ruta. Autorizar micrófono y Accesibilidad para la instancia concreta que se usa; las autorizaciones anteriores del ejecutable o paquete pueden no cubrir la nueva aplicación.

El paquete release se compiló y firmó localmente con Xcode 27.1. La interacción física de la aplicación sigue pendiente de verificación. Verificación estática, tests de widgets y compilación Rust no demuestran foco, vidrio, Dock, Stage Manager ni audio físico de la nueva UI.

## Datos y capacidades

El historial local conserva mensajes finalizados y chats anteriores. Consultar otro chat no cambia el hilo activo; renombrar solo cambia el nombre local y eliminar no borra el hilo del proveedor. Cerrar la ventana macOS la oculta y mantiene los servicios; Salir cierra coordinador, hijos y SQLite. Las migraciones SQL existentes permanecen inmutables, incluidas las preferencias de apariencia que ya no tienen consumidor visual. No se persiste audio.

Nemotron usa un proceso helper separado y pesos verificados bajo `~/Library/Application Support/AIModels/gguf`. El dictado mantiene sus preferencias y límites. Cada transcripción final se entrega a Codex; Laya no clasifica ese flujo. Kokoro narra las respuestas cuando está activado y conserva su entorno Python local y preferencias. [Dictado](docs/decisions/transcription.md), [conversación](docs/decisions/llm-conversation.md), [voz](docs/decisions/tts.md) e [historial](docs/decisions/chat-history.md).

[AGENTS.md](AGENTS.md), [UI.md](UI.md), [arquitectura](docs/architecture/overview.md), [módulos](docs/architecture/modules.md) y [verificación](docs/rules/testing.md) describen el estado vigente. Los registros de decisiones y evidencia anteriores documentan el sistema que existía entonces; sus sondas visuales retiradas no verifican Flutter. `.creation` conserva el origen sin cambios. Commit, push y despliegue requieren autorización expresa.

Los lanzadores admiten `CARGO_TARGET_DIR` para aislar la caché Rust; el valor predeterminado es `target/flutter`. Por ejemplo, `CARGO_TARGET_DIR=/tmp/gennie-flutter-target ./scripts/dev.sh --build-only`. Conservar el mismo directorio al compilar biblioteca y helper; evitar procesos Cargo simultáneos sobre él.

Evidencia del paquete release y la prueba del backend incluido: [build macOS](docs/verification/flutter-desktop.md).

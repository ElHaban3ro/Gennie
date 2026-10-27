# Arquitectura vigente

Gennie combina escritorio Flutter y backend Rust local mediante flutter_rust_bridge 2.13.0. El primer objetivo es macOS; Windows y Linux no están verificados. No hay servidor HTTP entrante, buses ni credenciales nuevas. [Mapa de módulos](modules.md) y [reglas](../rules/modules.md).

```text
frontend/lib → API FRB (bootstrap/src/api.rs) → runtime.rs
bootstrap → presence, transcription, decisions, llm, tts, reminders-plugin, assignments, browser, plugins, proactivity, mcp
browser → core, mcp
plugins → browser, llm, core, db, mcp
proactivity → core, reqwest
assignments → core, db, mcp
llm → mcp
mcp → core
presence → llm, core, db
transcription / decisions / llm / tts → core, db
reminders-plugin → core
core → config, db
```

Flutter posee la ventana, widgets y event loop. La biblioteca `gennie_backend` se carga en su proceso; un coordinador Rust serializa operaciones y consulta controladores fuera del hilo visual. Bootstrap construye AppState, valida el DAG, migra todos los módulos compilados, inicia los activos y detiene recursos antes de cerrar la única conexión SQLite. Kernel no importa apps ni UI.

`apps/presence` conserva historial, preparación de respuestas y narración. Su diario se inyecta al worker LLM mediante `ConversationJournal`; activación local y de proveedor comparten transacción. Mensajes finalizados se guardan antes de confirmarse. Leer otro chat no cambia el activo. [D-024](../decisions/chat-history.md).

El binario Rust `gennie` es helper separado para captura/inferencia y descarga Nemotron, no un escritorio. No usar el ejecutable Flutter como helper. Codex conserva su autenticación ChatGPT; cada dictado final llega a su conversación. TTS mantiene Kokoro y su proceso local. Laya sigue registrado sin entrar al flujo normal de voz. Recordatorios conserva su controlador EventKit sin conexión a conversación/UI.

Pesos compartidos en `~/Library/Application Support/AIModels`, entornos propios de cada app. [Almacén](../decisions/model-storage.md). Datos y migraciones existentes permanecen; las preferencias antiguas de apariencia flotante ya no tienen consumidor visual. No hay cara, globos ni presencia flotante.

Desarrollo: `scripts/dev.sh`, biblioteca/helper debug en `target/flutter`, hot reload Dart. Release: `scripts/build.sh`, `frontend/build/macos/Build/Products/Release/Gennie.app`. El paquete incluye dylib/helper y licencia Inter. El build release y la firma local del runner se verificaron con Xcode 27.1; el comportamiento físico de la nueva interfaz sigue pendiente. [Verificación](../rules/testing.md).

[Encargos y MCP modular](../decisions/assignments.md): cada app aporta herramientas propias a una biblioteca común. Bootstrap expone las activas mediante el helper stdio y un socket Unix privado; Store usa la conexión compartida. El Codex aislado recibe su configuración automáticamente. Assignments registra objetivo, referencia de método y autorización, además de políticas contextuales de seguimiento. La evidencia técnica se resuelve desde el catálogo inyectado por bootstrap, sin scheduler, memoria, acceso propio a Gmail ni avisos.

[Navegador D-026](../decisions/browser.md): `apps/browser` comparte un contexto persistente Chromium entre el botón de chat y herramientas MCP de navegación/interacción. Node/Patchright se preparan explícitamente por usuario; el perfil queda en Application Support/Gennie, separado de SQLite y del navegador habitual. El módulo no abre ventanas al arrancar y las herramientas respetan el control de aplicaciones configurado.

[Plugins web D-027](../decisions/web-plugins.md): browser guarda recetas HTTP/DOM y bloques JavaScript programables, permite verificar y reutilizar funciones desde MCP/Rust y abre headless bajo demanda. El login se solicita solo si la operación lo necesita y la sesión existente no basta. El constructor aislado y los mensajes al chat de origen están implementados; no hay scheduler; [plan](../plans/web-plugins.md).

Proactivity añade un cliente de decisiones de OpenRouter bajo demanda, con modelos Jev y OpenAI seleccionados por el llamador. La clave es opcional, separada de AppState; arrancar no consulta la red ni observa encargos.

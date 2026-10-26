# Mapa de módulos

| Componente | Responsabilidad |
|---|---|
| frontend/lib | Escritorio Flutter, navegación, historial, compositor y ajustes; API FRB |
| gennie-config | Config/Secrets, .env y validación |
| gennie-db | Una conexión SQLite compartida, transacciones y migraciones verificadas |
| gennie-core | AppModule, AppState y Shutdown; independiente de apps/frontend |
| gennie-presence | Datos de historial, ConversationJournal, preparación de respuestas y narración; sin ventanas |
| gennie-transcription | Preferencias, atajo, micrófono, descarga verificada y helper Nemotron |
| gennie-decisions | Catálogo, intención, disponibilidad, Laya y ejecutores locales comprobados |
| gennie-llm | Preferencias, worker de conversación, proceso Codex app-server y thread_id |
| gennie-tts | Preferencias y worker Kokoro MLX; audio no persistido |
| gennie-mcp | Biblioteca MCP reutilizable, registro de herramientas y transporte privado sin datos de negocio |
| gennie-browser | Contexto Playwright persistente y proceso Node propios; apertura FRB y herramientas MCP comparten el controlador, sin tablas |
| gennie-assignments | Planes y políticas contextuales de seguimiento persistidos, validación y herramientas de registro; sin ejecución ni avisos |
| gennie-reminders-plugin | Controlador y observador EventKit local; sin consumidor UI/IA |
| bootstrap / gennie_backend | Composición, API FRB, runtime serial y secuencia de cierre |
| gennie (binario) | Helper separado de audio/inferencia y herramientas locales; no ejecuta escritorio |

El grafo es acíclico; composición en bootstrap. Presence depende de llm para el trait público del diario, core y db. Bootstrap inyecta ese diario y conecta transcription, llm y tts. Los detalles de proveedor no entran en widgets. [Reglas de módulos](../rules/modules.md).

Presence mantiene las migraciones históricas y catálogo `presence_conversations`, `presence_current_conversation`, `presence_history`; no borrar tablas de apariencia ni alterar checksums por retirar su interfaz. Bootstrap posee `modules`; cada app conserva sus preferencias. Kernel no posee tablas de producto.

Flutter recibe snapshots y ejecuta operaciones asíncronas de `bootstrap/src/api.rs`. `runtime.rs` posee controladores y workers; cierra recursos antes de DB.close. La biblioteca en proceso y el helper separado se construyen juntos. Captura/inferencia no intentan ejecutar el runner Flutter como si fuera Rust.

Dictado envía frases finalizadas a llm; Laya permanece separado. La selección de historial es de lectura y no reactiva un hilo. Renombrar/eliminar son locales; Codex conserva sus hilos. La narración usa texto de respuesta y cola acotada, sin persistir audio. [Historial](../decisions/chat-history.md), [conversación](../decisions/llm-conversation.md), [voz](../decisions/tts.md).

La interfaz flotante, sus adaptadores y sondas visuales se retiraron. Los registros históricos describen esa implementación anterior y no prueban el escritorio Flutter. Linux/Windows pendientes; macOS necesita Xcode completo para el runner.

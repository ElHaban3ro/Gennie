# Gennie — instrucciones del checkout

Gennie es un asistente personal con escritorio Flutter local en macOS y backend Rust mediante flutter_rust_bridge, configuración y SQLite. Conserva dictado Nemotron, conversación con Codex y narración Kokoro; Laya permanece como módulo independiente. La presencia flotante, la cara y los globos fueron retirados y no deben replicarse. No hay HTTP entrante, usuarios ni correo integrado. Proactivity permite clasificación remota explícita mediante OpenRouter. Antes de modificar código, identifica las áreas afectadas y lee los archivos de reglas correspondientes. Los enlaces de este índice son instrucciones de lectura, no sustituyen su contenido.

## Mapa e invariantes
- El nombre visible de la aplicación se escribe **Gennie**, siempre con G mayúscula. Mantén en minúscula los identificadores técnicos, crates, comandos, rutas y nombres de archivo existentes.
- `kernel/config`: carga y valida Config; Secrets separado, sin Debug ni inclusión en AppState.
- `kernel/db`: única conexión SQLite compartida, transacciones y migraciones inmutables.
- `kernel/core`: AppModule, AppState y señal de apagado. Kernel nunca importa apps ni contracts.
- `frontend/lib`: interfaz Flutter, navegación, historial, conversación y ajustes; consume únicamente la API Rust generada.
- `kernel/mcp`: biblioteca de protocolo, contexto de cancelación y transporte local; cada app aporta sus herramientas contextuales para IO, sin tablas ni módulo MCP monolítico. Ver D-045.
- `apps/browser`: navegador Patchright/Chromium visible o headless bajo demanda, perfil persistente exclusivo, recetas web, diario propio de intentos en la SQLite compartida y herramientas MCP sobre la misma instancia; sin HTTP/CDP ni perfil personal. Ver D-026, D-027 y D-044.
- `apps/plugins`: constructor Codex aislado bajo demanda, candidatos verificados y trabajos/outbox entregados al bot para el chat de origen; configuración/catálogo/permisos de MCP externos, sin ejecución periódica. Ver D-027, D-028 y D-029.
- `apps/assignments`: registro de objetivo, método y autorización, políticas/horarios y preparación; borradores con guía consentida/aplicación humana (D-030), refinamiento por respuesta real sin renovar guía (D-035), archivo/páginas (D-036) e inicio local desde una petición del chat sin guía (D-037), MCP y SQLite propios; ejecución local explícita con calendario/diario y adaptador de bootstrap (D-031) y entrega de resultados por el bot elegida por el usuario (D-032); valoración contextual opt in mediante guía→bot y retirada local (D-034); piloto comparado y extensión a los diez casos semánticos ejecutados (D-038), con fallos conservados y revisión humana acotada, sin formato ganador demostrado.
- `apps/presence`: datos de historial, fragmentos terminales con estado explícito (D-047), preparación de respuestas y narración; conserva sus migraciones SQL históricas, sin interfaz ni ventanas.
- `apps/transcription`: dictado, preferencias propias, descarga verificada de Nemotron y proceso hijo de audio/inferencia con descarga por inactividad.
- `apps/proactivity`: cliente de decisiones OpenRouter bajo demanda para Jev y OpenAI; sin observadores ni rutinas automáticas.
- `apps/decisions`: catálogo, intención semántica y disponibilidad separadas, Laya local/adaptaciones, ejecutores comprobados y diario de intentos; sin reintentos automáticos.
- `apps/llm`: arquitectos base y catálogo ampliable de instancias/tipos reutilizables por ID (D-057), ajustes por instancia, inbox durable de delegaciones y mediación del constructor al chat (D-029/D-034; [rutas narrativas actuales auditadas](docs/verification/architect-communication.md)), proceso hijo app-server, sesión ChatGPT de Codex CLI, un hilo conversacional activo y su identificador SQLite, más sesiones técnicas aisladas sin alterar ese hilo; presence posee el catálogo de conversaciones e historial local, sin audio persistido. Bootstrap inyecta su diario al worker LLM; avisos atribuidos de la misma sesión se incorporan al proveedor antes del siguiente envío explícito, con recibos y recuperación conservadora (D-033); ver D-024/D-033.
- `apps/reminders-plugin`: controlador local de Recordatorios mediante EventKit; lectura, escritura y observador de cambios, sin conexión a la IA o UI todavía.
- `bootstrap`: biblioteca `gennie_backend`, API FRB en `api.rs`, coordinador en `runtime.rs` y binario helper de audio; composición explícita y secuencia de arranque/cierre. Grafo acíclico; datos de apps no invaden AppState.
- No existen contracts, HTTP, buses ni testkit sin una necesidad real. Los avisos futuros salen después del commit.
- Símbolos, archivos y SQL en inglés; comentarios y documentación en español. Ausencias con Option y fechas persistidas ISO 8601 UTC.

## Lectura obligatoria
| Condición | Documento | Motivo |
|---|---|---|
| Servicios Rust, errores, recursos compartidos, workers | [backend](docs/rules/backend.md) | Inyección, errores y concurrencia |
| UI, estilos, foco, animación o interacción | [UI](UI.md) y [frontend](docs/rules/frontend.md) | Decisiones visuales acordadas, Flutter y comportamiento nativo |
| Apps, kernel, Cargo, features, registro, contratos, lifecycle | [modules](docs/rules/modules.md) | Límites y dependencias |
| SQL, repositorios, conexión, transacciones o migraciones | [database](docs/rules/database.md) | Propiedad e inmutabilidad |
| .env, variables, secretos o preferencias | [configuration](docs/rules/configuration.md) | Precedencia y validación |
| Pruebas o alcance de verificación | [testing](docs/rules/testing.md) | Evidencia proporcional |
| Cambio transversal o app nueva | [overview](docs/architecture/overview.md) y [modules](docs/architecture/modules.md), además de reglas afectadas | Diseño actual |
| Continuación o decisión histórica | [memory](docs/memory/index.md) y [decisions](docs/decisions/README.md) | Contexto verificado |
| Creación incompleta o revisión de origen | Leer íntegros los cuatro `.creation/*.md` | Requisitos originales antes de generar código |

## Colaboración y operación
- Conserva la numeración del usuario al responder listas; separa temas cuando ayude.
- Corrige problemas incidentales claros y pequeños dentro del alcance autorizado e informa. Entrega primero el arreglo mínimo; explica mecanismos o arquitectura que amplíen el alcance antes de incorporarlos.
- Ante fallos inspecciona síntoma y causa con evidencia. No defiendas el resultado sin comprobarlo; distingue hechos, hipótesis y límites de acceso.
- Preserva cambios existentes, rama y checkout. Otro worktree o rama necesita petición/autorización expresa.
- Antes de arrancar un proceso indica propósito y puerto si aplica; no detengas procesos ajenos. Identifica y apaga tus procesos temporales al terminar, salvo petición de dejarlos activos. El producto no abre puertos; las herramientas Flutter de depuración pueden usar servicios locales temporales.
- Commit y push solo con autorización explícita para ese cambio. Convención de commits: mensajes descriptivos en español con formato `tipo: resumen` (docs, feat, fix), elegida para la entrega inicial autorizada. Publicar código y desplegar son acciones distintas; una autorización Git no autoriza despliegue.
- Distingue revisión estática, compilación, pruebas, ejecución y despliegue. No sustituyas evidencia runtime por una captura o un test unitario.
- Documenta estado vigente, actualiza documentos afectados y retira afirmaciones obsoletas. Si se autorizan agentes paralelos asigna archivos/responsabilidades y exige verificación propia; no hay obligación de delegar.
- Un único proceso Cargo por target. Lanzadores: `./scripts/dev.sh` y `./scripts/build.sh`. El binario Rust es un helper, no inicia la UI. Comandos de verificación: `cargo fmt --all -- --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`.

## Mantenimiento
Cada regla tiene un propietario en docs/rules. Cambia regla, referencias y comprobaciones en el mismo trabajo. Usuario y entorno prevalecen; una memoria histórica no es regla vigente. Resuelve contradicciones antes del cambio dependiente. Si falta una lectura obligatoria, informa y resuelve antes de modificar ese ámbito. `.creation` conserva origen/historial; los documentos consolidados en docs son la fuente vigente tras completar la inicialización.

# soc — instrucciones del checkout

soc es un asistente personal futuro; esta base ofrece presencia nativa local en macOS, configuración y SQLite. No hay HTTP, usuarios, sesiones, voz, correo ni conectores de IA. Antes de modificar código, identifica las áreas afectadas y lee los archivos de reglas correspondientes. Los enlaces de este índice son instrucciones de lectura, no sustituyen su contenido.

## Mapa e invariantes
- `kernel/config`: carga y valida Config; Secrets separado, sin Debug ni inclusión en AppState.
- `kernel/db`: única conexión SQLite compartida, transacciones y migraciones inmutables.
- `kernel/core`: AppModule, AppState y señal de apagado. Kernel nunca importa apps ni contracts.
- `apps/presence`: UI Slint, estado visual, preferencias propias y worker de persistencia.
- `bootstrap`: composición explícita y secuencia de arranque/cierre. Grafo acíclico; datos de apps no invaden AppState.
- No existen contracts, HTTP, buses ni testkit sin una necesidad real. Los avisos futuros salen después del commit.
- Símbolos, archivos y SQL en inglés; comentarios y documentación en español. Ausencias con Option y fechas persistidas ISO 8601 UTC.

## Lectura obligatoria
| Condición | Documento | Motivo |
|---|---|---|
| Servicios Rust, errores, recursos compartidos, workers | [backend](docs/rules/backend.md) | Inyección, errores y concurrencia |
| UI, cara, estilos, foco, animación o interacción | [frontend](docs/rules/frontend.md) | Slint y comportamiento nativo |
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
- Antes de arrancar un proceso indica propósito y puerto si aplica; no detengas procesos ajenos. Identifica y apaga tus procesos temporales al terminar, salvo petición de dejarlos activos. Esta app no abre puertos.
- Commit y push solo con autorización explícita para ese cambio. Convención de commits: mensajes descriptivos en español con formato `tipo: resumen` (docs, feat, fix), elegida para la entrega inicial autorizada. Publicar código y desplegar son acciones distintas; una autorización Git no autoriza despliegue.
- Distingue revisión estática, compilación, pruebas, ejecución y despliegue. No sustituyas evidencia runtime por una captura o un test unitario.
- Documenta estado vigente, actualiza documentos afectados y retira afirmaciones obsoletas. Si se autorizan agentes paralelos asigna archivos/responsabilidades y exige verificación propia; no hay obligación de delegar.
- Un único proceso Cargo por target. Comandos reales: `cargo run --locked -p soc`, `cargo fmt --all -- --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`.

## Mantenimiento
Cada regla tiene un propietario en docs/rules. Cambia regla, referencias y comprobaciones en el mismo trabajo. Usuario y entorno prevalecen; una memoria histórica no es regla vigente. Resuelve contradicciones antes del cambio dependiente. Si falta una lectura obligatoria, informa y resuelve antes de modificar ese ámbito. `.creation` conserva origen/historial; los documentos consolidados en docs son la fuente vigente tras completar la inicialización.

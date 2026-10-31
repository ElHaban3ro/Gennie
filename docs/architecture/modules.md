# Mapa de módulos

| Componente | Responsabilidad |
|---|---|
| frontend/lib | Escritorio Flutter, navegación, historial, compositor y ajustes; API FRB |
| gennie-config | Config/Secrets, .env y validación |
| gennie-db | Una conexión SQLite compartida, transacciones y migraciones verificadas |
| gennie-core | AppModule, AppState y Shutdown; independiente de apps/frontend |
| gennie-presence | Datos de historial, ConversationJournal, preparación de respuestas y narración; sin ventanas |
| gennie-transcription | Preferencias, atajo, micrófono, descarga verificada y helper Nemotron |
| gennie-proactivity | Cliente de decisiones OpenRouter bajo demanda para Jev y OpenAI, sin rutinas automáticas |
| gennie-decisions | Catálogo, intención, disponibilidad, Laya y ejecutores locales comprobados |
| gennie-llm | Preferencias, worker de conversación, proceso Codex app-server y thread_id |
| gennie-tts | Preferencias y worker Kokoro MLX; audio no persistido |
| gennie-mcp | Biblioteca MCP reutilizable, registro/contexto de cancelación y transporte privado sin datos de negocio |
| gennie-browser | Contexto Patchright persistente y proceso Node propios; apertura FRB y 24 herramientas MCP comparten el controlador; referencias, imágenes, HTTP de sesión, artifacts y recetas verificadas; diario SQLite de intentos web (D-044) |
| gennie-plugins | Constructor Codex aislado bajo demanda, candidatos y trabajos/outbox entregados al bot para el chat de origen; sin scheduler |
| gennie-assignments | Planes, propuestas y políticas de ejecución local explícita, calendario/diario y herramientas de registro; resultados por el bot elegidos por el usuario |
| gennie-reminders-plugin | Controlador y observador EventKit local; sin consumidor UI/IA |
| bootstrap / gennie_backend | Composición, API FRB, runtime serial y secuencia de cierre |
| gennie (binario) | Helper separado de audio/inferencia y herramientas locales; no ejecuta escritorio |

Cada encargo conserva una identidad de instancia independiente de su número y revisión. Los trabajos de preparación usan esa identidad para no confundirse con trabajos de encargos eliminados; los trabajos anteriores solo se asocian si coinciden el origen y el objetivo y su creación es posterior a la del encargo.

El grafo es acíclico; composición en bootstrap. Presence depende de llm para el trait público del diario, core y db. Bootstrap inyecta ese diario y conecta transcription, llm y tts. Los detalles de proveedor no entran en widgets. [Reglas de módulos](../rules/modules.md).

Presence mantiene las migraciones históricas y catálogo `presence_conversations`, `presence_current_conversation`, `presence_history`; no borrar tablas de apariencia ni alterar checksums por retirar su interfaz. Bootstrap posee `modules`; cada app conserva sus preferencias. Kernel no posee tablas de producto.

Flutter recibe snapshots y ejecuta operaciones asíncronas de `bootstrap/src/api.rs`. `runtime.rs` posee controladores y workers; cierra recursos antes de DB.close. La biblioteca en proceso y el helper separado se construyen juntos. Captura/inferencia no intentan ejecutar el runner Flutter como si fuera Rust.

Dictado envía frases finalizadas a llm; Laya permanece separado. La selección de historial es de lectura y no reactiva un hilo. Renombrar/eliminar son locales; Codex conserva sus hilos. La narración usa texto de respuesta y cola acotada, sin persistir audio. [Historial](../decisions/chat-history.md), [conversación](../decisions/llm-conversation.md), [voz](../decisions/tts.md).

La interfaz flotante, sus adaptadores y sondas visuales se retiraron. Los registros históricos describen esa implementación anterior y no prueban el escritorio Flutter. Linux/Windows pendientes; macOS necesita Xcode completo para el runner.

Encargos separa objetivo, referencia de método y autorización. Bootstrap inyecta un proveedor del catálogo web a Store; la resolución técnica es calculada, sin dependencia de crates entre assignments/browser ni modificaciones a permisos. Plugins expone sus hitos persistidos también en la ficha.

Presence también posee la identidad persistente del asistente inicial y la asociación de sus sesiones desde 0012. El repositorio `assistants` permite personalizar el nombre y consultar mensajes paginados entre sesiones sin reactivar hilos; FRB expone ese perfil, su renombrado y páginas del historial. Flutter utiliza el perfil en la cabecera y permite editar su nombre; el lector continuo consume páginas entre sesiones con procedencia y marcadores propios, y la sidebar presenta al asistente en lugar de sesiones. Los snapshots de compatibilidad también limitan sus mensajes. Se conservan el diario, selección y catálogos por sesión durante la transición.


Presence posee también citas y estabilidad de IDs de mensajes (0013). LLM recibe su contenido validado mediante el diario existente; bootstrap transmite IDs y compone API/worker sin añadir dependencias inversas. El prompt citado pertenece a la entrada actual y no hidrata historiales ajenos. Flutter reutiliza MessageQuote para cita persistida y borrador y navega al original mediante páginas locales.


Presence posee también la copia de imagen del asistente (0014) y su preparación con límites; no conoce rutas de selección del usuario. Bootstrap entrega por FRB preparación sin persistencia, guardado transaccional del perfil y lectura de píxeles por revisión. Flutter posee el selector nativo y el borrador; los snapshots solo transportan la referencia de revisión, no imágenes. El perfil no añade contexto al proveedor ni dependencias de UI al kernel.

Presence posee la evidencia de llamadas por turno y mensaje (0015), su deduplicación/asociación transaccional y recuperación de pendientes. LLM traduce eventos del proveedor a metadatos acotados mediante ConversationJournal. Browser aporta el nombre publicado junto con identidad/función en su resultado estructurado; no escribe historial. Bootstrap traduce datos a DTO FRB y Flutter los presenta bajo el mensaje. No hay almacén de cuerpos de herramientas, conexión SQLite adicional ni datos nuevos en kernel. Sesiones técnicas y títulos efímeros quedan fuera de esta evidencia conversacional.


Browser posee el contrato y validación previa de parámetros de funciones web; conserva recetas anteriores y amplía descriptores sin migración SQL. El esquema de herramientas de browser también sirve al constructor. LLM usa esos metadatos para pedir datos faltantes en su conversación; no registra Encargos para cada uso puntual. Flutter presenta/valida el formulario del catálogo existente; no sustituye la validación del ejecutor ni resuelve identidades de servicios por su cuenta. El backend de fuentes MCP externas ya define configuración/lifecycle y catálogo/permisos (D-028); su gestión visual está integrada en Plugins y el recorrido nativo stdio tiene descubrimiento/permisos, validación, resultado perdido y revisión sin reenvío sobre datos ficticios; ver [cierre funcional](../verification/functional-closeout.md).


Plugins posee mcp_source/mcp_store/external_mcp: especificación validada, tabla 0006, permisos, catálogos y lifecycle. LLM expone mcp_client como adaptador de protocolo sin inferencia, reutilizando el arranque IPC con perfil propio y sin cuenta de ChatGPT enlazada. El hilo técnico es un recurso de transporte, no un bot/helper/guía. Bootstrap conserva plugin_request JSON y registra los dos callbacks de catálogo/ejecución para el bot. Kernel y su transporte local no adquieren datos de fuentes externas; el grafo permanece acíclico.

D-039 separa la lectura de permisos persistidos del bot de la existencia del worker LLM: bootstrap usa architects::runtime_settings para MCP manual. El constructor mantiene su requisito propio de LLM activo; no hay dependencias, tablas o preferencias nuevas.

D-041 compone los proxies MCP del bot con Plugins/LLM activos, sin exigir browser o assignments. Solo las herramientas web/de construcción conservan la condición de browser; el propietario de configuración/ejecución/diario no cambia.

Plugins posee además plugin_mcp_attempts (0007): reserva previa al efecto, recuperación conservadora y revisión humana con cambio de revisión. El gate de lifecycle espera escrituras finales tras cancelar transportes. Flutter consulta estos metadatos por la API JSON existente; no añade SQL o lógica de proveedor a widgets ni herramientas de autoaprobación al modelo.


D-029/D-057: llm::architects posee las bases llm_architects, el catálogo adicional llm_architect_instances, la vista conjunta llm_architect_profiles y el espejo histórico llm_preferences; valida identidades, revisión y permisos. Worker LLM guarda. Plugins recibe AgentSettings (usuario/helper separados), aplica gate de configuración/construcción y toma perfil efectivo por trabajo. Su outbox entrega una actualización estructurada a llm::delegation por bootstrap. Este módulo posee inbox/worker de redacción; presence confirma el mensaje y su atribución mediante ConversationJournal, dentro de la transacción de entrega. Cada app conserva su SQL. Proactivity recibe GuideProvider desde bootstrap, sin dependencia de llm ni acceso SQL propio; evalúa solo bajo demanda explícita con su cliente. No se añaden módulos kernel ni chats técnicos visibles. D-030 integra borradores y valoración de guía mediante callbacks de bootstrap, con tablas y resolución en assignments. Contexto remoto principal incorporado en D-033; rutas narrativas actuales auditadas y comprobaciones acotadas con proveedores reales registradas en el cierre funcional.


D-032: Assignments posee outbox de resultados explícitos, LLM un inbox separado de los trabajos del helper y Presence la referencia de ejecución del mensaje. Bootstrap adapta los hechos al bot; el worker serial existente redacta o conserva fallback fijo, sin herramientas. Fuente y recibo confirman antes del ACK, mensaje y recibo final juntos. No hay arquitecto ficticio para el ejecutor ni SQL entre propietarios. La vista de intento se comparte entre chat y ficha por FRB.

D-033: Presence posee recibos/cola de contexto por mensaje; el diario inyectado reserva entrada/cita/avisos en una transacción. LLM posee los datos tipados y el adaptador de thread/inject_items, sin importar Presence ni consultar sus tablas. ACK local antes del turno explícito, recuperación conservadora sin repetir cuerpos. Assignments conserva la consulta de resultados y comprueba consentimiento/origen para su herramienta de lectura. No se amplía AppState ni se cambia FRB, Dart, procesos o grafo de dependencias.

D-034: Proactivity prepara una consulta con su configuración capturada; bootstrap adapta Guide/permiso para el worker de bot. Assignments posee opción contextual, contexto de política, consulta de actividad y retirada local; LLM posee recibos/consejo/decisión y solo el bot puede emitir mensajes. Orden opcional de lifecycle proactivity→llm, sin import entre esos crates o activar módulos. Entrega lista revalida permiso sin preparar modelo; contexto/errores/datos nunca cruzan SQL de propietario. FRB mantiene su forma y no se crean workers o AppModules.

D-035: Presence posee la lectura acotada de la última entrada real del usuario de la sesión activa; bootstrap adapta origen/intervención de la invocación MCP. Assignments posee versiones, extractos de auditoría, propuestas y aplicación humana, sin importar Presence/LLM. Refinar no hereda guía ni ejecuta; no cambia FRB ni crea workers.

D-042: Presence posee build_messages y su SQL de atribución; LLM define BotMessage junto a Origin. Bootstrap implementa BuildJournal::messages y Plugins proyecta bot_presentation en la API JSON de trabajos. Flutter presenta esa sección o estados fijos; no el informe técnico del helper. Sin migración, nuevo worker, dependencia inversa o activación de modelos durante lectura. Ver [mensajes del constructor](../decisions/builder-messages.md).

D-043: kernel/mcp ofrece with_content junto a with_image, sin importar apps. Plugins adapta la salida para el bot con bloques nativos y error original; la API humana mantiene el JSON original y el diario no cambia. La compactación conserva texto no redundante; no se incorpora un cliente de recursos. Ver [contenido MCP](../decisions/native-mcp-content.md).

D-044: Browser posee attempts/web_execution y browser_web_attempts (0001), con Database compartida inyectada por bootstrap. Preflight Node no ejecuta; Controller confirma reserva antes de IO y cierre después, conservando incertidumbre ante fallo. Plugins expone lectura/revisión humana mediante el JSON existente y observa la revisión del dueño. Flutter reutiliza la revisión MCP, sin SQL o nueva API FRB tipada. Encargos conserva referencias web/MCP sin cuerpos privados. Ver [diario web](../decisions/web-attempts.md).

D-045: kernel/mcp observa EOF con un lector por conexión unido y el helper multiplexa entrada/socket/salida. LLM acepta contexto en su cliente MCP sin inferencia; Plugins lo enlaza bajo mutex. Browser combina consumidor/cierre propio, retira el runtime con IO incierto y conserva su diario. Bootstrap/constructor registran variantes contextuales; no hay tablas, workers de negocio o API FRB nuevos. Tool tradicional no se vuelve preemptible por conversión. Ver [cancelación de transportes](../decisions/transport-cancellation.md).

D-047 añade llm::turn como módulo interno y una petición FRB stop_response por identidad. Bootstrap filtra eventos y detiene narración propia; Presence confirma el fragmento sellado y su estado en 0019; Flutter integra el control en el compositor actual. El cierre de módulo sigue drenando el worker antes de SQLite. Sin nuevo AppModule ni cambios en la barra general. Ver [Detener respuesta](../decisions/chat-cancellation.md).

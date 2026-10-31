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

La UI presenta el chat continuo de un único asistente, Gennie, con nombre e imagen editables. Presence vincula sus sesiones y pagina por fecha/ID, caracteres, mensajes y hasta tres sesiones distintas; no carga todo el historial ni lo convierte en contexto del modelo. Nueva conversación cambia el hilo del proveedor y conserva las anteriores para lectura. Las citas explícitas y los avisos atribuidos de la misma sesión tienen sus canales delimitados; ver [plan vigente](../plans/gennie-chats-agentes.md), [D-053](../decisions/quote-navigation-focus.md) y [D-054](../decisions/chat-session-budget.md).

El binario Rust `gennie` es helper separado para captura/inferencia y descarga Nemotron, no un escritorio. No usar el ejecutable Flutter como helper. Codex conserva su autenticación ChatGPT; cada dictado final llega a su conversación. TTS mantiene Kokoro y su proceso local. Laya sigue registrado sin entrar al flujo normal de voz. Recordatorios conserva su controlador EventKit sin conexión a conversación/UI.

Pesos compartidos en `~/Library/Application Support/AIModels`, entornos propios de cada app. [Almacén](../decisions/model-storage.md). Datos y migraciones existentes permanecen; las preferencias antiguas de apariencia flotante ya no tienen consumidor visual. No hay cara, globos ni presencia flotante.

Desarrollo: `scripts/dev.sh`, biblioteca/helper debug en `target/flutter`, hot reload Dart. Release: `scripts/build.sh`, `frontend/build/macos/Build/Products/Release/Gennie.app`. El paquete incluye dylib/helper y licencia Inter. Build, firma local y recorridos funcionales acotados están verificados. El usuario retiró todo trabajo adicional de accesibilidad/contraste y sus matrices; el [cierre funcional](../verification/functional-closeout.md) registra el paquete y sus límites. [Verificación](../rules/testing.md) y [auditoría del plan](../verification/plan-audit.md) separan versiones y alcance.

[Encargos y MCP modular](../decisions/assignments.md): cada app aporta herramientas propias a una biblioteca común. Bootstrap expone las activas mediante el helper stdio y un socket Unix privado; Store usa la conexión compartida. El Codex aislado recibe su configuración automáticamente. Assignments registra objetivo, referencia de método y autorización, además de políticas contextuales de seguimiento. La evidencia técnica se resuelve desde el catálogo inyectado por bootstrap. [D-031](../decisions/assignment-execution.md) añade ejecución local explícita con calendario y diario mediante Plugins; [D-032](../decisions/assignment-notices.md) permite entregar resultados al bot únicamente con política elegida por el usuario. No hay acceso propio a Gmail.

[Navegador D-026](../decisions/browser.md): `apps/browser` comparte un contexto persistente Chromium entre el botón de chat y herramientas MCP de navegación/interacción. Node/Patchright se preparan explícitamente por usuario; el perfil queda en Application Support/Gennie, separado de SQLite y del navegador habitual. El módulo no abre ventanas al arrancar y las herramientas respetan el control de aplicaciones configurado.

[Plugins web D-027](../decisions/web-plugins.md): browser guarda recetas HTTP/DOM y bloques JavaScript programables, permite verificar y reutilizar funciones desde MCP/Rust y abre headless bajo demanda. El login se solicita solo si la operación lo necesita y la sesión existente no basta. El constructor aislado y los mensajes al chat de origen están implementados; el constructor no ejecuta horarios. Esa función pertenece a Assignments bajo D-031; [plan del constructor](../plans/web-plugins.md).

Proactivity añade un cliente de decisiones de OpenRouter bajo demanda, con modelos Jev y OpenAI seleccionados por el llamador. La clave es opcional, separada de AppState; arrancar no consulta la red ni observa encargos.


[Fuentes MCP externas D-028](../decisions/mcp-plugins.md): backend de configuración/permisos/catálogo en Plugins y cliente stdio/HTTP bajo demanda de LLM, sin inferencia ni conversación del usuario. La tabla 0006 conserva los trabajos anteriores; no hay listener HTTP del producto ni conexiones al arrancar. plugin_mcp_catalog/plugin_mcp_run permiten uso por el bot bajo permisos guardados. Flutter integra conexión, permisos, ficha y pruebas manuales en Plugins; diario persistido de resultados inciertos y revisión humana integrados. Los límites vigentes de interacción nativa, semántica y equivalencias entre fuentes están en la [auditoría del plan](../verification/plan-audit.md).


Los arquitectos (D-029/D-057) pertenecen a llm: tres bases persistidas para bot/helper/guía y un catálogo ampliable con múltiples instancias y tipos personalizados, gestionado desde su pestaña propia. Los consumidores actuales conservan sus bases y nuevas integraciones pueden referenciar instancias por ID. Se conserva el inbox durable del bot para el outbox del constructor. Bootstrap inyecta perfiles y adapta la entrega; proactivity conserva su interfaz propia y sus secretos. El bot prepara avisos con contexto acotado de la sesión de origen; presence confirma mensaje y atribución junto con el recibo del inbox, sin cambiar la sesión activa. El constructor usa su perfil, el canal MCP el límite del usuario. Configuración no crea chats adicionales o trabajos ni envía consultas de guía. Habilitar el helper permite continuar trabajos previamente encolados y autorizados. D-030 incorpora selección de encargos, borradores del bot y valoración opcional de la guía, con aplicación humana transaccional. D-031/D-032 implementan ejecución local y resultados opt in; D-033 incorpora avisos en el contexto remoto principal de la misma sesión. D-034 integra valoración contextual opt in de preparación, mediada por el bot; las rutas narrativas actuales están auditadas y cuatro acuerdos ficticios posteriores recorrieron guía/bot reales y Presence. La comparación de propuestas cubre diez casos con revisión humana acotada y asistida diferenciadas, sin ganador general; ver el cierre funcional.


[D-032](../decisions/assignment-notices.md) añade entrega de resultados opt in: ejecución→outbox de Assignments→inbox/worker del bot→mensaje atribuido en Presence. Recuperación sin repetir función o inferencia. D-034 continúa la valoración contextual de preparación con consentimiento y retirada local. D-033 incorpora la entrega atribuida al contexto remoto principal de esa sesión antes del siguiente envío explícito.

[D-035](../decisions/proposal-refinements.md) continúa las propuestas de D-030 desde una respuesta real de la sesión activa: Presence conserva la entrada, bootstrap adapta su referencia y Assignments crea una versión auditable sin heredar guía ni modificar el encargo. El padre queda consultable y la aplicación sigue siendo humana. D-036 añade archivo y D-037 permite iniciar desde chat sin formulario de selección previo; la comparación empírica de diez casos está ejecutada, con una elección humana y revisión asistida separadas; no se afirma revisión humana amplia o superioridad de formato.

[D-036](../decisions/proposal-history.md) añade archivo humano por familia y consulta de revisiones actuales/archivadas por cursor de instancia/modo. Conserva documentos, borradores y recibos; la familia archivada deja de ser pendiente del bot y no puede continuar ni resolver borradores. Todo pertenece a Assignments y su API humana, sin workers, dependencias o cambios FRB nuevos. El inicio desde chat continúa en D-037; la comparación empírica mantiene sus resultados y límites en el cierre funcional.

[D-037](../decisions/chat-review-requests.md) permite inicio local desde una petición explícita del chat: bootstrap toma la fuente real de Presence y Assignments confirma revisión/recibo juntos, siempre sin guía. El bot selecciona referencias vigentes y la aplicación permanece humana. La UI lee procedencia por el mismo JSON FRB y observa metadatos de revisión sin mensajes artificiales. No se añaden AppModules, dependencias o workers.

[D-044](../decisions/web-attempts.md) añade el diario de intentos al ejecutor web de Browser sobre la misma SQLite. Las recetas/perfil siguen en archivos. La reserva precede al efecto, la incertidumbre sobrevive al reinicio y la revisión humana no ejecuta ni reactiva horarios. No añade procesos automáticos o conexiones al arrancar.

[D-045](../decisions/transport-cancellation.md) propaga la vida del consumidor MCP a sus llamadas Browser/MCP externo y al constructor. EOF o cierre del host se observan durante IO; se cancelan procesos propios y se mantienen los recibos inciertos. Browser anuncia cierre antes de esperar el mutex; cancelar su runtime compartido puede cerrar sus pestañas. Sin cambios visuales/FRB, control Detener o persistencia de parciales en esta etapa.

[D-047](../decisions/chat-cancellation.md) continúa D-045 con Detener en el compositor: identidad por entrada, cancelación del proceso conversacional propio y eventos filtrados. Presence 0019 conserva una vez el fragmento terminal con estado cancelled/failed; no persiste deltas. La UI conserva estilo/sidebar. Las pruebas locales cubren filtrado y persistencia; FRB real comprueba lectura y petición vencida. La sonda posterior3a5cb86e añade fragmento observado, corte local y reanudación del mismo hilo con proveedor real y diario doble, sin afirmar ACK de cancelación o coste remoto.

La [sonda posterior del proveedor](../verification/provider-chat-probe.md) verificó cita explícita y sesión nueva aislada con cuenta real. La primera fase de Detener quedó inconclusa y conserva su fallo; otra campaña posterior3a5cb86e verificó corte local y reanudación. Esos resultados no sustituyen las pruebas de Presence ni acreditan cancelación del servicio remoto.

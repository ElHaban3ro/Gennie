# Módulos y dependencias

Propietario del grafo, workspace, registro, AppModule y lifecycle. Workspace edición 2024, resolver 3, versiones centralizadas exactas; crates heredan package/dependencies. Kernel no importa apps/contracts. Apps pueden importar kernel y otras apps solo sin ciclos. Contracts solo cuando rompa un ciclo real; no generar capas/crates vacíos.

## Registro y orden
Registro explícito en bootstrap/registry.rs; no discovery de módulos por DB, red ni filesystem. AppModule Send+Sync expone nombre estable, activación inicial, dependencias obligatorias/opcionales, namespace persistido de migraciones y start/stop. El namespace usa name por defecto y puede conservar una identidad histórica al renombrar una app; no cambia su nombre vigente. No incluir HTTP, permisos o pestañas sin consumidor.

Nombres: minúsculas ASCII/dígitos/underscore; rechazar duplicados de nombre y de namespace de migraciones. No normalizar guiones, así no hay colisiones ambiguas. Validar DAG antes de migrar apps. Obligatorias ausentes son errores de código; apagadas son errores de configuración. Opcionales ausentes son válidas; presentes ordenan aunque estén apagadas. Fila huérfana no representa código disponible. Insertar módulo no sobrescribe selección persistida. Migrar todos los compilados, iniciar solo activos. No hay activación en caliente en este hito: modificar selección requiere reiniciar. Implementar hot activation solo con transición serializada, gate y compensaciones definidos si se solicita.

## Lifecycle y comunicación
Start no entra en el event loop, es idempotente y serializado; stop detiene solo recursos propios y es idempotente. Flutter inicia la biblioteca mediante la API FRB; el runtime Rust coordina start fuera del hilo visual. Si falla start, limpiar ese módulo y los iniciados en orden inverso, conservar error original y registrar secundarios. Salir solicita shutdown, detiene módulos y cierra DB al final; cerrar la ventana macOS la oculta y mantiene servicios.

Bootstrap consume Controllers públicos de transcription, llm, tts y decisions (este último solo para estado/carga manual); presence conserva datos y helpers de respuesta/narración. Decisions permanece registrado, pero no se conecta a Presence ni recibe transcripciones. `gennie-llm` posee la conversación de Gennie y aísla el proveedor Codex en `codex.rs`; bootstrap compone los Controllers en runtime.rs. Para nuevas comunicaciones: llamada pública si dependencia real acíclica; trait/DTO en contracts para ciclo; bus para hechos sin respuesta. Un broadcast es efímero y acotado: manejar lag y reconstruir desde fuente, publicar después de commit, no prometer entrega durable. No convertir tablas de otras apps en interfaz implícita.

`gennie-tts` se compone en bootstrap sin dependencia visual de presence. Posee su Controller y recursos propios; ver [D-022](../decisions/tts.md).

`gennie-reminders-plugin` es una app independiente registrada en bootstrap, sin dependencias de otras apps ni tablas. Su Controller bloqueante debe consumirse fuera de main. El observador EventKit pertenece al módulo y se detiene antes del cierre; ver [diseño](../decisions/reminders-plugin.md).

## Dependencias
Antes de añadir/actualizar: verificar versión exacta en registro oficial y mantenedor, MSRV/plataforma/features/licencia y avisos RustSec o del mantenedor. Guardar fuentes, fecha y límites en docs/decisions/dependencies.md. Lockfile versionado; no afirmar ausencia de vulnerabilidades porque no haya avisos. Revisar tránsito y artefacto objetivo; no habilitar Qt, servidores, red o tray sin necesidad. La API FRB se genera desde bootstrap/src/api.rs; revisar conjuntamente código Rust y Dart al cambiarla.

## Añadir una app
1. Registrar requisito y responsabilidad, tablas propias, dependencias y recursos en plan/decisión vigente.
2. Leer architecture/overview y modules, backend y testing; database/UI/configuration según alcance.
3. Crear crate significativo con interfaz pequeña, no andamiaje vacío; añadir dependencias centrales y registro.
4. Validar ciclos, activación, migraciones incluso apagada, lifecycle y limpieza tras fallo.
5. Actualizar mapa/docs en el mismo cambio. Composición solo en bootstrap; SQL y secretos según documentos propietarios.

## Encargos y MCP

`browser` aporta herramientas compatibles con Playwright mediante Patchright mediante el mismo registro. Bootstrap inyecta un único Controller compartido entre AppModule, API FRB y MCP; no construir un navegador independiente por consumidor. Perfil propio, recetas y bloques programables, y Node/Chromium pertenecen a la app; D-044 añade únicamente su diario de intentos web en SQLite compartido. Las recetas se ejecutan por la misma interfaz desde MCP y Rust, con verificación por revisión; ver [D-027](../decisions/web-plugins.md). Start no abre ventanas; stop cierra su contexto y proceso, conservando el perfil. Ver [D-026](../decisions/browser.md).

`assignments` posee planes/validación/tabla y aporta herramientas a `kernel/mcp`, biblioteca reutilizable sin registro AppModule ni datos de negocio. Bootstrap registra las herramientas de módulos activos después de migrar y configura llm antes de start. Cada app futura podrá aportar herramientas propias sin introducir un módulo MCP monolítico. El helper stdio solo reenvía al host privado; comparte la conexión DB por inyección en el Store del host. Cierre de clientes/host antes de DB. Ver [D-025](../decisions/assignments.md).

El transporte MCP admite resultados de imagen de herramientas propias mediante with_image y bloques nativos aportados por adaptadores mediante with_content; mantiene separado structuredContent y no conoce apps ni páginas. Requests siguen en 128 KiB y respuestas en 4 MiB. Los artifacts y la lectura/subida de archivos pertenecen a browser, sin acceso a tablas de otras apps. Ver D-026.

Si la duplicación del resultado como texto excede el límite MCP, el transporte conserva structuredContent completo y sustituye solo el texto duplicado por una referencia breve. Si aun así no cabe, devuelve response_too_large con resultado de operación desconocido, sin cerrar el canal ni autorizar repetirla. Server expone si conserva clientes conectados, sin conocer trabajos o decidir reintentos; plugins usa esa señal entre turnos para programar recuperación de sus propias sesiones.

`plugins` se compone explícitamente en bootstrap; posee trabajos/outbox y un worker serial bajo demanda, con diario y ajustes inyectados. Usa browser y las sesiones builder públicas de llm sin importar presence; bootstrap adapta el diario hacia el inbox del bot. Presence conserva mensajes y atribución de entrega, sin consultar tablas de jobs. Kernel no importa plugins. Stop cancela y une el constructor antes de browser/MCP/SQLite. Ver [D-027](../decisions/web-plugins.md) y D-029.

Proactivity es una app independiente, compuesta en bootstrap con la clave opcional; no depende de assignments, plugins ni decisions local, y no ejecuta rutinas al arrancar.


Plugins posee también fuentes MCP externas (D-028) y sus permisos/catálogo en 0006. LLM aporta únicamente el cliente de protocolo sobre el app-server ya integrado: stdio/HTTP bajo demanda sin inferencia o diario. No se añade un módulo MCP con tablas al kernel ni dependencia inversa. Los callbacks del chat registran catálogo/ejecución sin permitir configuración o concesión de permisos desde el modelo. Stop de Plugins cancela/unifica sus clientes antes de SQLite; no conecta servidores al arrancar ni al leer metadatos guardados. Flutter integra gestión y prueba manual de fuentes; su verificación nativa permanece pendiente.

D-039: bootstrap inyecta al transporte MCP la lectura pública del perfil persistido del bot, independiente del worker conversacional. Se conservan los límites de permisos; no iniciar LLM para descubrir o probar una fuente manual. El constructor conserva su gate de LLM activo. No consultar SQL de perfiles desde Plugins ni añadir una interfaz alternativa de configuración.

D-041: registrar plugin_mcp_catalog/plugin_mcp_run con Plugins y LLM activos aunque browser/assignments estén apagados. Las herramientas web y del constructor conservan su condición de browser activo. El inventario deriva de esa composición explícita; no activar módulos o conceder permisos para satisfacerlo.

El diario 0007 de Plugins coordina reservas y revisión de llamadas MCP, sin mover datos al kernel. El cierre del servicio tiene un gate de operaciones: rechaza entradas nuevas, cancela clientes propios y espera confirmaciones antes de DB. Las herramientas del modelo publican lectura/ejecución, no mcp_reconcile; esta acción pertenece a la API humana local.


LLM posee architects y delegation como módulos internos: perfiles e inbox durable, sin nuevo AppModule ni contrato vacío. Bootstrap inyecta los perfiles efectivos al constructor y GuideSettings al cliente de proactivity; esta app no importa llm ni consulta sus tablas. Plugins mantiene separados ajustes del usuario para MCP y del helper para construcción; entrega su outbox al bot mediante el adaptador de bootstrap. El bot prepara el mensaje y presence lo confirma mediante ConversationJournal en la transacción de entrega. Cada propietario conserva su SQL. Kernel no conoce roles. D-030 conecta propuestas de assignments con un callback de proactivity desde bootstrap, sin nuevas dependencias entre apps. Rutas narrativas actuales auditadas y casos de guía/bot reales comprobados con sus límites; ver [cierre funcional](../verification/functional-closeout.md).


D-031: Assignments posee Runner, calendario y SQL de ejecución; bootstrap inyecta el Controller existente de Plugins por un trait público. Sin dependencia inversa ni nuevo AppModule. Activación solo por API humana, un Runner serial/exclusivo por base. Stop antes de cancelar módulos/transportes, drenar entrada y unir worker antes de DB. Con Plugins apagado solo consulta/pausa/revisión; no habilitar módulos automáticamente.


D-032: llm::notices es el consumidor tipado de hechos de ejecución, dentro del worker de bot existente. Bootstrap adapta Notice desde Assignments y entrega al Receiver; Presence implementa el diario. El ejecutor no es un arquitecto nuevo. No añadir dependencia assignments→llm ni consultas SQL entre apps. Runner con Plugins apagado puede atender solo el outbox si conversación está activa; mantiene exclusión/cierre sin habilitar módulos.

D-033 mantiene contexto y recibos en los propietarios existentes: LLM define PreparedUser/Context y el protocolo del proveedor; Presence implementa preparación/confirmación por el diario inyectado y posee 0018. Assignments valida la lectura de resultados contra origen inyectado. Sin dependencias nuevas, nuevo worker ni cambios al puente. Ninguna app lee tablas ajenas para incorporar avisos.

D-034 conserva guía/decisión en el worker e inbox de LLM (0009); bootstrap implementa el adaptador de valoración y permisos entre Proactivity y Assignments. LLM declara dependencia opcional de lifecycle con proactivity para iniciarlo antes de atender pendientes y detenerlo después; sin import Rust de esas apps ni activación forzada. La guía solo aconseja al bot, sin herramienta o entrega propia. Retirada y lecturas de actividad pertenecen a Assignments; no se expone revocación por MCP. Las salidas listas consultan el permiso sin preparar otro modelo. No reentrar DB desde la transacción de entrega.

D-035: Presence expone entrada real acotada por hilo/sesión activa; bootstrap la adapta a las herramientas de propuestas. Assignments posee versiones/extractos de revisión y aplicación humana sin importar Presence/LLM; no añadir SQL entre apps, otra conexión, worker o contrato vacío. La guía no se hereda al crear un sucesor.

D-043: with_content pertenece a kernel/mcp como formato genérico sin conocimiento de Plugins; el adaptador de Plugins conserva identidad/diario en structuredContent y mueve solo content a bloques nativos. No añadir resolución automática de recursos, permisos o datos de fuentes al kernel. with_image y sus consumidores existentes mantienen formato.

D-044: Browser declara/inicia su migración propia de intentos web; bootstrap inyecta Database mediante with_database antes del arranque para permitir lectura con el módulo apagado. Start idempotente recupera reservas; Controller serializa preflight/reserva/IO/confirmación y revisión. Plugins adapta consulta/revisión por su JSON humano y suma execution_revision para refrescar; no consulta SQL de Browser. No se añade AppModule, conexión, worker o listener.

D-045: kernel/mcp posee CallContext/ContextTool y multiplexación/IO cancelable sin importar apps. Browser enlaza cierre propio y Plugins propaga contexto a su cliente MCP; el constructor registra herramientas contextuales. Bootstrap compone esas variantes explícitas; FRB y Tool tradicional conservan su API. Detener un consumidor no cancela clientes independientes; cancelar un runtime Browser compartido puede cerrar sus pestañas. No añadir estado de negocio al kernel. Diseño y límites en [cancelación](../decisions/transport-cancellation.md).

D-047: llm::turn posee identidades/contextos de Speak; bootstrap coordina stop_response y filtra eventos por identidad. Presence posee interruption y su migración 0019; Flutter solo consume la API generada. Sin AppModule, dependencia inversa, endpoint HTTP o ejecutor nuevos. Cancelar conversacional no cancela guía/constructor/manual; EOF mantiene los límites de D-045. Ver [decisión](../decisions/chat-cancellation.md).

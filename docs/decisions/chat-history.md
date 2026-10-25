# D-024 — Historial propio de conversaciones

> Contexto histórico: la UI descrita aquí fue retirada al migrar a Flutter. La cara, presencia flotante, globos y sus sondas no forman parte del producto vigente. Para arquitectura y verificación actuales, consultar los documentos de arquitectura y reglas del índice. Las migraciones y los datos conservan su historia.

2026-10-24. Gennie conserva y consulta su historial local. La sidebar no lee los archivos grandes de Codex ni requiere conectar con el proveedor para mostrar mensajes guardados.

## Avisos de sistema en el lector — 2026-10-25

Los errores de operaciones locales, conversación, dictado y voz se registran desde el coordinador como mensajes `system` mediante presence, antes de publicar el snapshot. Los eventos asíncronos pertenecen al chat activo; los fallos de acciones locales al chat seleccionado. Se conserva la selección de lectura. Los errores que ya guardó el worker de conversación se reconocen por el último mensaje de ese chat y no se duplican al recibir Failed; Disconnected actualiza únicamente el estado de conexión. Dos fallos locales iguales en momentos distintos conservan ambas ocurrencias.

Si aún no hay conversación, el aviso crea un historial local «Avisos del sistema», sin hilo del proveedor ni activación de modelos. No se reactivan chats eliminados. La escritura del mensaje y la fecha de actividad son transaccionales; no cambian migraciones. Si falla el guardado o la lectura posterior, el error sigue visible en el chat como «Sistema · sin guardar» y no se declara persistido. `Snapshot.error_recorded` comunica ese resultado al frontend; `error` se conserva para los diálogos y el resultado de la operación. El puente Rust/Dart se regenera junto al contrato.

Las confirmaciones rutinarias no crean mensajes ni llenan los chats nuevos vacíos. Los ajustes mantienen sus confirmaciones dentro del diálogo; el lector deja de dibujar la banda superior de avisos.

## Filas Flutter — 2026-10-25

«Nuevo chat» reutiliza el chat activo si sigue visible y sin mensajes persistidos de ningún emisor, incluso después de desconectar o reiniciar. El worker serial de llm consulta el diario de presence antes de conectar o solicitar otro hilo al proveedor; devuelve la confirmación habitual para que el escritorio muestre ese chat aunque se estuviera consultando otro. Un título editado no convierte un chat vacío en uno con contenido. Chats eliminados o con mensajes permiten crear uno nuevo. No se borran duplicados anteriores ni se modifican migraciones.

Verificación local: 27 pruebas de llm/presence y cinco del coordinador correctas, con tres pruebas reales de Codex omitidas. La regresión del coordinador reabre SQLite temporal y procesa diez solicitudes desde la lectura de otro chat, conservando catálogo, hilo y contador de escrituras. No abre la interfaz, modelos ni una conexión al proveedor; no verifica el binario de la instancia en uso.

La fuente vigente del diseño es [UI.md](../../UI.md). `ChatHistoryTile` presenta título seminegrita, fecha local relativa y vista previa de una línea; ambas líneas están limitadas por el ancho y la fila mide 64 px. El menú de acciones locales se abre con clic derecho o teclado, sin seleccionar la conversación; desaparece el botón de tres puntos.

El catálogo incluye `last_message: Option<String>` mediante una subconsulta de presence sobre el índice existente `(conversation_id,id)`: toma el último mensaje por orden de inserción, sin filtrar emisor. Entrega como máximo 512 caracteres Unicode para la vista previa y conserva íntegro el historial. La UI normaliza espacios y aplica elipsis. No se añaden tablas ni migraciones, consultas al proveedor, escrituras o cambios de selección; la fecha procede de `updated_at`, que renombrar no modifica. El contrato se propaga por API/runtime y puente FRB regenerado.

Los apartados siguientes conservan la evolución histórica de la interfaz anterior; sus referencias a filas de una línea, puntos y Slint no describen este diseño.

Desde el 25 de octubre de 2026, el lector Flutter abre el historial al final, tanto en la carga inicial como al cambiar de conversación. Sustituye el inicio arriba descrito en el alcance histórico. Las entradas nuevas siguen el final solo si el usuario no se desplazó para leer mensajes anteriores; ver [UI.md](../../UI.md).

## Alcance histórico

Cada conversación tiene un ID local, fechas UTC y un vínculo opcional de proveedor/hilo. El nombre visible prioriza una edición local, después el nombre de Codex y finalmente «Nuevo chat» para chats vinculados sin nombre (los registros históricos sin proveedor conservan su título). La sidebar ordena por última actividad e ID descendentes. Los chats nuevos mantienen «Nuevo chat» hasta recibir un nombre del proveedor o una edición del usuario. Se guardan texto escrito o dictado enviado al bot, cada respuesta final por separado y los errores nuevos como actividad de sistema. No se guardan deltas, razonamiento, herramientas, estados temporales ni audio; el Markdown y las referencias del mensaje final se conservan íntegros.

Seleccionar una fila muestra sus mensajes, emisor y fecha en el área principal. La consulta comienza arriba al cambiar de chat, permite scroll y no modifica el chat activo. Esa selección es transitoria. El panel flotante existente continúa mostrando y enviando al chat activo. Las filas ocupan una sola línea de 36 px, sin hora ni fecha, con título elidido y menú de tres puntos horizontales. Permiten renombrar o eliminar solo en Gennie. Reanudar para conversar desde un chat seleccionado, buscar e importar JSON históricos quedan fuera de esta entrega.

## Propiedad y atomicidad

Presence posee `presence_conversations`, `presence_current_conversation` y `presence_history`. `gennie-llm` define la interfaz pequeña `ConversationJournal`; bootstrap inyecta la implementación de presence al construir LlmModule. No hay dependencia de llm hacia presence, tablas ajenas consultadas implícitamente, nueva conexión ni cambios en AppState.

Activar un hilo invoca el diario con la conexión prestada dentro de la transacción que guarda `codex_conversation`. Crear o encontrar la conversación, cambiar la selección local y guardar el ID del proveedor hacen commit o rollback juntos. El callback no obtiene de nuevo el mutex de Database. Solo después del commit se cambia el ID de la sesión en memoria y se anuncia el cambio. Si falla guardar el nuevo chat, el ID anterior permanece persistido y la siguiente conexión lo reanuda; no se borra su historial.

Los mensajes se escriben desde el worker serial LLM, identificados por proveedor/hilo; el guardado del usuario precede a `turn/start` y su fallo impide enviar. Cada respuesta final se guarda antes de notificarla a la UI. Si falla ese guardado, se conserva su texto en el globo y se comunica el fallo, sin fingir persistencia ni repetir la petición. Los errores se guardan como `system` cuando existe una sesión identificada. Si falla conectar, el borrador escrito permanece visible. No se garantiza recuperar un dictado rechazado antes de establecer una sesión; no hay audio persistido ni importación de su texto desde archivos del proveedor.

Un contador compartido avisa de commits; el worker de lectura consulta SQLite fuera de main. Las cargas incluyen la identidad de la selección solicitada para descartar resultados obsoletos al cambiar rápidamente de fila. El lector no reconstruye mensajes si su contenido no cambió. Los errores de lectura y escritura se muestran en el escritorio, incluso con la presencia flotante oculta. El borrador escrito solo se retira al confirmar guardado y si todavía coincide con el texto enviado.

El cierre del worker LLM precede al cierre compartido de DB; las escrituras de un mensaje son síncronas en ese worker, sin una segunda cola de escritura que pueda perderse al cerrar la UI. El worker de lectura termina y se une al liberar el historial.

## Migración

La migración 0008 de presence no modifica las anteriores. Copia las filas existentes, conservando IDs, emisor, texto y fechas, a una sola conversación «Historial anterior», sin proveedor/hilo. No convierte separadores textuales en vínculos comprobados. Una base sin mensajes no crea una conversación antigua vacía. Cada mensaje nuevo exige un ID de conversación válido mediante FK; hay índices para leer en orden dentro de un chat y listar por actividad.

## Verificación y límites

Pruebas con DB propia: conservación de datos antiguos y preferencias, ausencia de vínculos inventados, FK, aislamiento de conversaciones, títulos Unicode, respuestas/referencias intactas, errores como sistema, selección de lectura independiente, reapertura, confirmación después del commit y rollback del proveedor/catálogo ante fallo local.

`cargo run --locked -p gennie-presence --example conversation_history_probe -- .verification/conversation-history` usa el árbol Slint real con fixtures y renderer de software, sin ventanas nativas, DB, modelos, audio, red ni puertos. Comprueba clic/teclado, desplazamiento con mensajes largos, ancho mínimo y Plugins; genera capturas para inspección del texto completo. No prueba la composición nativa de Liquid Glass, entrada física ni una nueva conversación real con ChatGPT. Las pruebas del proveedor que consumen suscripción siguen siendo optativas e ignoradas por defecto.

Resultados locales del 24 de octubre: `cargo test --workspace --locked` pasó 106 pruebas y dejó 6 optativas ignoradas; `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --locked -p gennie`, formato, invariantes y diff sin errores. La sonda nueva y `sidebar_probe` pasaron. Se inspeccionaron capturas de conversación, final del texto largo, ventana mínima y modo oscuro después de terminar la transición de paleta, sobre fondos opacos de prueba. No se inició ni reinició una instancia de Gennie, no se migraron datos del usuario y no se ejecutaron pruebas reales con ChatGPT.

## Nombre del proveedor y acciones locales

La migración 0009 añade nombres de proveedor/local y un marcador de eliminación, sin tocar 0008 ni otras migraciones aplicadas. El nombre inicial de `thread/start` o `thread/resume` se guarda en la misma transacción de activación. El adaptador procesa `thread/name/updated` para el hilo correcto y consulta `thread/read` con `includeTurns=false` después de un turno, con un plazo de tres segundos y sin cargar el historial del proveedor. El título generado se guarda con `thread/name/set`; no se envía `thread/delete` ni `thread/archive`. La [documentación oficial de OpenAI](https://learn.chatgpt.com/docs/app-server) establece que `thread.name` puede ser nulo y que la lectura de metadatos no reanuda el hilo; el esquema generado por el CLI instalado confirma `threadId` y `threadName` nullable en la notificación. Si Codex no entrega un nombre, Gennie genera uno con una consulta temporal tras guardar el primer intercambio, según la ampliación del 25 de octubre descrita abajo.

Renombrar acepta entre 1 y 200 caracteres Unicode, normaliza espacios y escribe solo `local_title`. El nombre editado sobrevive a actualizaciones del proveedor y reaperturas; los mensajes y el orden por actividad permanecen iguales. El menú no selecciona la fila. El editor conserva su borrador cuando falla el guardado y recibe foco después de cerrar el menú, permitiendo Intro/Escape.

Eliminar borra las filas de mensajes y limpia los nombres en una transacción; mantiene solo la referencia marcada para impedir que una respuesta en curso la haga reaparecer. No se borra ni se renombra el hilo de Codex. Si era el chat activo, su contexto en el proveedor sigue existiendo: una nueva entrada explícita del usuario crea de nuevo un registro visible con únicamente los mensajes nuevos, sin recuperar los eliminados. El catálogo excluye referencias eliminadas y una selección retirada pasa a otro chat visible o al estado vacío.

Pruebas adicionales: prioridad de nombres y ausencia de cambios en mensajes/actividad, validación sin escritura parcial, reapertura, guardado confirmado desde worker, eliminación sin cambios de proveedor, respuesta tardía ignorada y rollback real de una eliminación fallida mediante trigger de SQLite. La sonda Slint ejecuta las dos acciones del menú con fixtures, edición con Intro y cancelación con Escape; no modifica datos del usuario ni demuestra una nueva conversación real con Codex.

Verificación de la ampliación: 111 pruebas del workspace pasaron (6 optativas ignoradas), Clippy con `-D warnings`, build de Gennie, formato e invariantes correctos. La sonda ejecutó «Cambiar nombre» y «Eliminar chat» desde el menú real de Slint con callbacks de prueba, Intro para enviar la edición y Escape para cancelar; se revisaron las capturas claras/oscuras con filas de una línea y selección translúcida. Las pruebas SQLite cubrieron persistencia y rollback reales en bases propias. No se reinició Gennie ni se probaron títulos con un turno real de ChatGPT.

## Marco modal compartido — 2026-10-24

El formulario de nombre deja de dibujar una tarjeta dentro de DesktopWindow. Compone `GennieModal` y se presenta mediante `modal::Surface`, una hoja nativa de la ventana propietaria en macOS con material de vidrio compartido. El controlador conserva callbacks y persistencia existentes: los nombres siguen siendo locales, el error conserva el borrador y el éxito cierra tras commit. Las reglas del componente común viven en [UI.md](../../UI.md) y [frontend](../rules/frontend.md); el menú y el formulario tienen sondas separadas en [testing](../rules/testing.md).

Verificación del modal: la sonda software pasó Intro/Escape, botones, bloqueo durante guardado y borrador ante error, con capturas claras/oscuras y a 300 px de ancho. La sonda nativa pasó asociación de hoja, foco, material NSGlassEffectView real, geometría, cierre/reapertura y callback de envío. Sobre franjas nativas de 20 px se observó transparencia y suavizado dentro del marco; retirar el marco de documento modal de AppKit evitó una base blanca opaca. Esta prueba no mide refracción ni equivale a un guardado con el proveedor. La captura usa solo la identidad de la ventana propia y puede no estar disponible si macOS la retira de pantalla. No se reinició la instancia del usuario.

## Menú contextual compartido — 2026-10-24

Por petición del usuario, el menú de los puntos se dibuja exclusivamente mediante `GennieContextMenu` de Slint en macOS, Windows y Linux. Se retiró la adaptación NSMenu y su sonda; no hay selector ni caminos visuales alternativos. `ContextAction` contiene identificador, título, SVG y Keys; cada acción muestra lápiz o papelera a la izquierda y el atajo a la derecha. El nombre del chat y un separador encabezan la lista.

Los KeyBinding del FocusScope del popup producen ⌘R/⇧⌘⌫ en macOS y Ctrl+R/Ctrl+Shift+Retroceso en Windows/Linux; dejan de actuar al cerrar. Las acciones usan los callbacks locales existentes después de cerrar el popup. No cambia proveedor, SQL ni contenido. La sonda compartida y sus límites están descritos en [testing](../rules/testing.md).

## Títulos sin usar el primer mensaje — 2026-10-25

El contenido enviado no se utiliza como título. La lectura prioriza `local_title`, después `provider_title` y, para un chat vinculado sin nombre, «Nuevo chat». También ignora el título provisional guardado por versiones anteriores; conserva nombres locales, mensajes, fechas, selección y títulos de registros sin proveedor. No modifica migraciones ni requiere reescribir datos existentes.

Se mantiene la recepción del nombre desde `thread/start`, `thread/resume`, `thread/read` y `thread/name/updated`. Tras la autorización del usuario del 25 de octubre, si Codex no entrega nombre se hace una consulta adicional en un hilo efímero para titular el primer intercambio; hasta recibir un título válido se conserva «Nuevo chat». La cabecera limita la presentación a una línea de hasta 320 px con elipsis; el nombre completo permanece en el catálogo y en el editor.

Verificación del cambio: 31 pruebas Rust de llm/presence y 18 pruebas de widgets correctas; tres pruebas reales del proveedor omitidas. Clippy de ambos crates, análisis Dart de los archivos afectados, formato, invariantes y diff sin errores. No se inició ni reinició Gennie ni se verificó un nombre recibido en una conversación real con Codex.

## Generación de títulos — 2026-10-25

Después de un turno correcto y de consultar el nombre del proveedor, el worker LLM reserva una generación mediante `ConversationJournal::claim_title_input`. Presence devuelve el primer mensaje de usuario y la primera respuesta posterior solo si el chat existe, no está eliminado y carece de nombre local y del proveedor. La migración 0010 añade `title_generation_attempted`; la reserva confirma ese indicador en una transacción antes de consumir inferencia. Un fallo o reinicio posterior no genera reintentos automáticos. Los chats anteriores sin nombre pueden obtenerlo al completar su siguiente turno, usando su primer intercambio guardado. No se recorre ni modifica el catálogo en segundo plano.

La consulta usa el modelo seleccionado y el menor esfuerzo que anuncie su catálogo (`none`, `minimal`, `low` y sucesivos), independiente del esfuerzo configurado para conversar. Si el catálogo no contiene un esfuerzo reconocido, se omite la generación. Usa un `thread/start` efímero separado del hilo activo y herramientas/comandos/web desactivados. El contexto se limita a 1.200 caracteres del usuario y 4.000 de la respuesta, sin cortar UTF-8. `apps/llm/src/title.rs` contiene instrucciones, construcción del prompt y validación: español, hasta siete palabras y 80 caracteres, sin explicación ni formato; un saludo no debe inventar un tema. El turno temporal tiene un plazo de generación de 10 segundos, aparte de las esperas IPC acotadas. En fallo del turno se solicita interrupción; se retira la suscripción temporal al terminar. El app-server administra la liberación del hilo efímero tras su plazo de inactividad o al cerrar el proceso propio.

El título válido se escribe en Codex mediante `thread/name/set` y en `provider_title`, anunciando el cambio después del commit. No se añade la petición ni su resultado a `presence_history`, ni se narra el título. El nombre local conserva prioridad, incluso si se edita durante la generación. La consulta y la actualización del título son opcionales: sus fallos se descartan sin avisos ni reintentos y conservan «Nuevo chat» y la respuesta ya guardada. Los cambios de API FRB no son necesarios.

Verificación: pruebas aisladas del flujo IPC cubren éxito y rechazo del proveedor, respuesta conservada, fallos silenciosos y ausencia del título entre mensajes. También comprueban que un chat con esfuerzo `high` solicita el título con `none` cuando el catálogo lo admite. SQLite temporal comprueba reserva única, nombres existentes, eliminación y reapertura sin reintento. Una consulta real de título con `gpt-6-luna`/`low`, «Qué onda pa» y la respuesta de ejemplo «¡Qué onda! ¿Cómo vas?» devolvió «Saludo informal en español»; se confirmó con `thread/read`, sin cambiar el hilo activo ni añadir mensajes al diario de prueba. El proceso y directorio temporales se cerraron/eliminaron. No se reinició ni se comprobó la ventana real de Gennie.

Ajuste posterior del 25 de octubre: la prueba real anterior usó `low`; el esfuerzo mínimo por catálogo y los fallos silenciosos se verificaron después con pruebas aisladas, sin otra consulta de suscripción. La selección no presupone que todos los modelos permitan desactivar razonamiento; `supportedReasoningEfforts` es la fuente de capacidades de cada modelo ([API de Codex](https://learn.chatgpt.com/docs/app-server)).

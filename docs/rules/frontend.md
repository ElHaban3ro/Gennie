# Interfaz Flutter

Alcance: `frontend/lib`, assets y runners de plataforma. Leer [UI.md](../../UI.md), [modules](modules.md) y [configuration](configuration.md) antes de cambiar sus ámbitos. Flutter posee el event loop y los objetos visuales; Rust no crea ventanas. No reconstruir presencia flotante, cara ni globos.

## Límites

El frontend consume la API de `bootstrap/src/api.rs` mediante flutter_rust_bridge 2.13.0. No editar archivos generados manualmente: regenerar con la configuración raíz y revisar Rust/Dart juntos. El coordinador `bootstrap/src/runtime.rs` posee módulos y controladores, serializa operaciones y cierra recursos antes de SQLite. `apps/presence` conserva únicamente historial, preparación de respuestas y narración.

No SQL, procesos de modelos ni lógica de proveedor en widgets. Las operaciones son asíncronas y comunican errores visibles. Confirmar guardados después del commit; conservar borradores ante fallo. No hacer fallback silencioso si falta backend, modelo o configuración.

## Diseño e interacción

Mantener el nombre Gennie, icono propio, Inter 4.1 local, tamaños y navegación de [UI.md](../../UI.md). Widgets compartidos para menús y diálogos, etiquetas semánticas, teclado y foco visible. La selección de lectura no modifica el hilo activo. Estado transitorio como pestaña y ancho permanece en Dart; preferencias persistidas cruzan la API Rust.

Todos los tooltips deben usar `AppTooltip` de `frontend/lib/src/glass.dart`, también en ajustes, menús y acciones nuevas. No usar `Tooltip` de Material ni textos en la propiedad `tooltip` de los controles; para `PopupMenuButton`, dejar `tooltip: ''` y envolverlo con `AppTooltip`. Conservar un único tooltip visible, radio 8 px, sin borde ni sombra; el formato compacto usa Inter 12/500 y padding 14 × 7, material nativo HUD en macOS y filtro Flutter compartido en otros destinos. Mantener semántica, presentación por foco/hover y cierre al salir, pulsar o Escape. La colocación se adapta al control y al espacio disponible. Los controles de modelos usan contenido estructurado compartido por Flutter y macOS: padding de 16 px, título Inter 13/600, estado con punto de color y etiqueta Inter 11/500, punto medio y RAM Inter 11/400. Solo ante fallo se muestra «No se pudo cargar el modelo.» en Inter 12/400, sin diagnóstico concreto; el pie Inter 11/400 solo informa del plazo de inactividad, separado por línea y 10 px. La medición de RSS se solicita únicamente al abrir el tooltip por hover; nunca desde build, actualizaciones de snapshots o temporizadores. Esta variante conserva material, radio y reglas de cierre; no simular la jerarquía mediante saltos dentro de una única etiqueta. El resto conserva el formato compacto 14 × 7. `scripts/check_invariants.py` impide tooltips Material añadidos por separado; los tests de widgets comprueban el uso común.

Cuando un único IconButton tenga nombre/estado en un Semantics exterior, unir ese subárbol mediante MergeSemantics: la acción efectiva, nombre, selección/alternancia, habilitación y foco deben pertenecer al mismo nodo accesible. No fusionar acciones independientes. AppDetailHeading aplica esa unión solo al botón Editar; el nombre no puede quedar en un contenedor separado de la acción. Los IconButton cuyo icono aporta semanticLabel, como Recargar plugins, deben demostrar nombre, rol y tap en el mismo nodo. Un fallo del catálogo Browser debe atribuirse al catálogo web, sin describir las fuentes MCP disponibles como inaccesibles. RailButton y Abrir Gennie Web conservan sus estilos, tamaños y callbacks; comprobar el nodo nombrado y ejecutar su acción semántica, además de conservar el teclado y bloqueo durante IO.

Cerrar, minimizar y redimensionar corresponden a la ventana macOS. La integración nativa vive en el runner Flutter. No presentar transparencia dibujada como prueba de Liquid Glass nativo. Los enlaces de respuestas se abren solo por acción explícita del usuario.

## Desarrollo y verificación

Al combinar accesibilidad del sistema y preferencias propias en GennieApp, conservar `MediaQuery.disableAnimationsOf(context)` mediante OR con reducedMotion local. No sustituir la señal heredada por false mientras falta snapshot o la preferencia está apagada. Probar un consumidor real y cambios de ambas señales sin persistir ajustes o iniciar modelos; no inferir de ello que el embedder de macOS transmita un ajuste no observado. SystemMotion recibe por gennie/system-motion el valor AppKit inicial antes de runApp y sus notificaciones posteriores; GennieApp lo combina por OR con la señal heredada y la preferencia local. El runner no escribe ajustes. Retirar su observador al aprobar Salir, conservándolo ante rechazo; el stream no inicia backend/modelos ni introduce sondeos.

`./scripts/dev.sh` prepara Rust y abre Flutter debug con hot reload; cambios Rust requieren recompilar. `--build-only` no abre UI y no compila el runner. `--generate` regenera el puente. `./scripts/build.sh` crea el paquete release con dylib, helper y licencia; no despliega.

Revisar análisis y tests Dart, tests Rust pertinentes y estado del código generado. El build macOS exige Xcode completo. Si no está disponible, informar del bloqueo: ni análisis ni widgets prueban ventana, composición, entrada física o permisos del sistema. Ver [testing](testing.md).

## Sidebar compartida

Chats, Encargos, Plugins y Arquitectos deben usar `AppSidebar` de `frontend/lib/src/sidebar.dart`. El ancho y el agarre pertenecen al escritorio y se conservan entre pestañas; no crear una sidebar fija independiente por módulo. Búsqueda con `SidebarSearch`, filas con `SidebarTile`, vacío lateral con `SidebarEmpty` y vacío del lector con `ContentEmpty`. Las diferencias de contenido no deben introducir otras lupas, escalas de fuente, fondos ni estructuras de búsqueda. SidebarTile usa 64 px como altura mínima y permite crecer con texto ampliado; no recortar las dos líneas mediante una altura fija. Cabecera y placeholder usan el gris informativo por tema de los estados vacíos; el subtítulo predeterminado conserva tinta al 62 % en oscuro y 80 % en claro para mantener contraste sobre el material, también seleccionado. Los overrides de subtítulo se conservan. No cambiar geometría, iconos o barra vertical por este ajuste.

Los logos de Encargos son recursos de presentación del runner (`gennie/assignment-logo`), derivados de la aplicación principal guardada por Rust. Flutter no resuelve apps, hace HTTP ni consulta contenido de servicios. El runner usa iconos instalados/favicons acotados y caché regenerable; no abre sesiones. El calendario representa al encargo y el icono de la aplicación aparece en un círculo superpuesto; si no se obtiene imagen, se usa su inicial. Ver [diseño](../decisions/assignments.md#recursos-visuales-de-las-fuentes).

## Vistas de detalle compartidas

Encargos y Plugins usan `AppDetailView` de `frontend/lib/src/detail_view.dart` para cabecera de 56 px, título, acciones, aviso y cuerpo. Lectura, construcción y edición usan `AppViewScroll`; `AppViewContent` aplica el ancho máximo de 760 px **incluidos** los márgenes interiores de 24 px. Los pies fijos usan el mismo contenedor con 12 px verticales. No repetir este marco ni aplicar el padding antes del límite de ancho en cada módulo. Las secciones y operaciones siguen perteneciendo a cada vista.

`AppDetailHeading` comparte icono, etiqueta, título, subtítulo y acción de edición entre las fichas de Encargos, Plugins y trabajos en preparación.


## Chat continuo

Chats presenta asistentes con AppSidebar/SidebarTile y mantiene los estilos de la barra vertical. ChatTimeline consume exclusivamente AssistantChatBackend/FRB para sus páginas (máximo tres sesiones distintas, junto con el límite de mensajes/caracteres); no utilizar Snapshot.messages como historial visual completo. Las revisiones de mensajes disparan actualización del tramo solicitado, con ancla visible si se están leyendo mensajes anteriores. Páginas anteriores conservan IDs y la sesión de cada mensaje para acciones de plugins. Las marcas de sesión proceden del backend, no de diferencias entre emisores o de avisos tardíos. La lectura no modifica la sesión activa; enviar siempre usa el worker de conversación vigente. Mostrar fallos de lectura y reintento sin convertirlos en mensajes de sistema persistidos.


## Responder y citas

ChatMessage usa MessageActions para Responder con clic secundario, Shift+F10, tecla de menú y acción semántica; el foco visible no cambia la geometría. MessageActions mantiene siempre su capa de decoración, vacía cuando no dibuja borde: retirar esa capa al perder foco desmonta los controles hijos e impide volver a un enlace después de cerrar un diálogo. La prueba de retorno debe incluir ChatMessage/MessageActions, además del widget de detalle aislado. MessageQuote es compartido por burbujas y previsualización cancelable del compositor. El envío de respuesta transmite solo texto e ID original por FRB; el backend resuelve y valida el contenido. Conservar borrador y selección ante fallo o ack de otra cita. Al pulsar una cita disponible, el lector carga páginas y localiza el original sin seleccionar/reactivar su sesión. Al localizar una cita, entregar el foco a las acciones del mensaje original después del scroll; Mayús–F10 debe responder a ese mensaje, sin activar su sesión. Los nodos se crean solo para destinos visitados, se retiran después de desmontar los hijos y se liberan al cerrar el lector. Una navegación posterior invalida el foco pendiente anterior. Si el original desaparece, conservar el fragmento y mostrar «Mensaje original no disponible» sin enlace activo. Nuevas piezas conservan Inter, tintes, radios y la barra general vigente.

## Perfil e imagen

AssistantProfileDialog conserva nombre e imagen como borrador local y guarda ambos por AssistantProfileBackend/FRB. El selector file_selector solo lee el archivo elegido, con tope de bytes antes y durante la lectura; cancelar no invoca guardado. Rust prepara/valida la copia antes de presentarla. La UI nunca abre SQLite ni persiste rutas. El diálogo admite scroll del formulario con título/acciones fijos, foco, bloqueo mientras prepara/guarda y error con borrador intacto. DismissIntent permite cancelar con Escape cuando está libre, conservando el bloqueo del fondo. El FocusScope interior mantiene accesible esa acción después de deshabilitar los controles por IO; no cerrar mientras prepara/guarda. Probar retorno al invocador y ciclo Tab/Mayús–Tab. Su base tonal local mantiene contraste sin alterar Glass ni la barra general. Ver D-051. El reintento invalida la caché antes de construir/observar el nuevo Future, para que una lectura fallida repetida no quede sin observador. Antes de guardar el nombre, rechazar localmente vacío/blanco, saltos de línea y más de 200 escalares Unicode, manteniendo la validación Rust independiente. Un rechazo local no invoca DesktopState.perform ni debe generar un aviso persistido en el chat. Devolver foco al nombre y desplazar el grupo campo/error después del layout; respetar movimiento reducido. Campo/error y Guardar/Cancelar deben seguir visibles al mínimo con texto ampliado.

AssistantAvatar comparte tamaños y SVG del pingüino existentes; DesktopState comparte una única lectura de bytes por revisión entre cabecera/fila/diálogo. Una revisión nueva invalida la imagen anterior, sin mostrar una respuesta tardía de otra revisión. Fallo visible con reintento desde el diálogo; la cabecera mantiene el arrastre de ventana. Restaurar el pingüino solo se aplica al guardar. No cambiar el estilo ni la geometría de la barra vertical.

## Evidencia de herramientas

ChatTimeline entrega ChatEntry.tools a ChatMessage; ToolEvidence agrupa metadatos recibidos por FRB bajo la burbuja. No derivar uso de herramientas del contenido del mensaje. Diferenciar llamadas pendientes, completadas, fallidas y desconocidas, incluyendo estados mixtos; cuando estén vinculadas a la solicitud identificarlas como actividad del asistente. El diálogo muestra una instantánea de los detalles al abrir, con título/acciones fijos, número de llamadas y scroll explícito; enlaces con teclado/foco y AppTooltip. Abrir un plugin web exige servidor local, browser_plugin_run e ID recibido. Una fuente MCP externa usa exclusivamente el alias mcp:<id> validado; no la resuelve por coincidencia de nombre. No publicar argumentos, resultados, errores libres ni IDs de correlación. Mantener componentes y navegación generales.

## Parámetros de funciones

FunctionProbe presenta el contrato validado del catálogo mediante PluginParameter: tipos anteriores requeridos y descriptores con etiqueta/descripción, opcionales, elecciones y límites. Formularios con etiqueta permanente, ayuda y errores junto al campo; teclado numérico según tipo, booleano/enum sin valor elegido por defecto. Opcionales ausentes se omiten; false y cero son valores explícitos. Node vuelve a validar antes del efecto: la comprobación Dart no sustituye ese control.

Validar durante edición y al ejecutar; enfocar/desplazar el primer error, conservar borrador ante fallo y bloquear campos/envío mientras espera. input_required/invalid_arguments con executed=false admite errores por campo del backend. Cambiar datos retira el resultado previo. No alterar barras, materiales ni geometría general para ampliar el formulario existente.

## Gestión de fuentes MCP

McpSourceEditor/McpSourceView usan los marcos de Plugins, sin cambiar la barra general. Guardar, descubrir, habilitar y conceder herramientas son acciones separadas. Guardar no inicia procesos; permisos y revisión se confirman desde Rust. Conservar borradores y enfocar/desplazar el primer error, también en opciones avanzadas plegadas. El pie de edición permanece visible. Ficha estrecha omite el icono grande sin modificar AppDetailHeading compartido.

McpToolProbe reutiliza FunctionProbe para campos simples y conserva acceso a JSON avanzado/estructuras complejas. La validación Dart no reemplaza JSON Schema del host. Bloquear doble envío y cambios de selección/permisos mientras espera; presentar estado, fallo e incertidumbre sin equivalencia con éxito. Resultados pendientes procede del diario Rust y persiste al cambiar de ficha/herramienta o reiniciar. El diálogo de revisión no preselecciona applied/not_applied, conserva elección/errores y bloquea cierre/envío mientras guarda; confirmar no ejecuta. Cerrar revisión conserva borradores de permisos y argumentos. El probador también se bloquea por pendientes recibidos al abrir y no ofrece una liberación solo local. Lista paginada de cincuenta con total/cursor y herramientas retiradas visibles; recargar metadatos por revisión de actividad, sin sustituir contratos o borradores, y descartar respuestas tardías. No persistir resultados manuales ni exponer valores de credenciales en formularios de variables.


## Configuración de arquitectos

ArchitectsView/ArchitectSettingsPanel consumen únicamente ArchitectBackend/FRB en su pestaña propia. Reutilizar AppSidebar, SidebarSearch, SidebarTile y AppDetailView; conservar la geometría/materiales de navegación al añadir su RailButton. La lista se identifica por ID, admite múltiples instancias por tipo y tipos libres, sin exigir exactamente tres filas. Las tres bases no se eliminan ni cambian de tipo/proveedor. Borradores por instancia, opciones avanzadas plegadas, errores y conflictos conservan campos. Crear/duplicar no guarda hasta confirmar; las nuevas instancias no sustituyen consumidores base ni activan modelos. Bloquear selección, navegación y doble envío mientras guarda. Configuración ya no contiene el panel; los ajustes históricos del asistente conservan su ruta. Ver D-057.

## Avisos del bot sobre trabajos

ChatEntry.delegation identifica trabajo, plugin y secuencia; presentar el cuerpo como mensaje assistant con las burbujas existentes. Si pertenece a otra sesión, mostrar «Actualización de una conversación anterior», sin reactivar esa conversación. Las acciones nuevas consultan get_job por ID y validan origen, plugin, última secuencia y sucesor vivo antes de habilitar continuar/reconstruir/ver plugin. No inferirlas de la similitud entre mensaje y resumen ni limitar la referencia al catálogo reciente. Mantener compatibilidad del texto solo para avisos históricos. El texto interno del helper no sustituye el mensaje del bot. Conservar estados de espera/error y bloquear doble envío; la API vuelve a comprobar permisos. No cambiar sidebar, material o geometría general para estos avisos.


## Propuestas de Encargos (D-030)

AssignmentProposals consume AssignmentProposalBackend/FRB dentro de la ficha. Mantener barra y marcos generales. El diálogo fija las referencias observadas al abrir; selección explícita de uno a veinte, orientación y guía remota desmarcada con explicación de datos compartidos. Guardar la selección y enviar petición natural al bot; no mostrar IDs técnicos ni decidir la valoración en Dart. Borrador/consentimiento conservados al fallar, foco y scroll del primer error. La revisión muestra valores actuales/propuestos, incluidos campos retirados; preguntas/contexto obsoleto impiden aplicar. Confirmar por la API humana, bloquear doble envío/cierre mientras guarda y mostrar error visible con borrador intacto. Títulos/acciones fijos y cuerpo con scroll al mínimo/texto ampliado; ninguna acción aplica por una probabilidad de guía.


D-031: ejecución en la ficha de Encargos, con formulario tipado según función, activación inicialmente apagada, revisión fija y error/borrador conservado. Los campos de horario, función, JSON y retraso tienen ancla/nodo propios, igual que los parámetros: enfocar el primer FormFieldState inválido en orden visual y desplazar después del layout, sin pedir foco genérico al diálogo. Liberar los nodos al desmontar. Validar por interacción en cada control para retirar el error al corregir, sin marcar de golpe campos intactos ni enviar configuración durante edición. AppReviewDialog comparte la superficie tonal de propuestas, título y acciones fuera del scroll; no cambia navegación ni tema general. Actividad por cursor y detalle de versión original. Revisión incierta sin opción inicial, nunca reanuda al comprobar; enlazar al diario MCP cuando corresponda sin afirmar resolución conjunta.

D-034: consentimiento remoto separado, desmarcado, visible solo para preparación con seguimiento y condicionado a destino elegido. Explicar datos compartidos/resultado sin aviso ante fallos; conservar elección y parámetros al fallar guardado. «Desactivar guía y avisos» permanece disponible con Plugins apagado o función en curso: es revocación local, no reconfiguración/ejecución. Actividad distingue registro sin aviso, pregunta/propuesta/aviso preparado, entrega pendiente y fallo de valoración; actualizar consulta sin ejecutar ni reconciliar. Mantener acciones fijas, scroll, foco y superficies existentes; ninguna modificación a sidebar o estilo general.

D-035: preguntas de propuestas ofrecen responder en el chat solo con origen vigente, bot y fuente verificable; navegar no envía ni aplica. Una versión anterior se identifica y bloquea nuevas resoluciones, con consulta de su sucesora para el mismo encargo. Durante esa consulta bloquear inspectores duplicados; ausencia/error queda visible. No borrar historial ni presentar una versión sustituida como edición externa. Conservar acciones fijas, scroll, estilo general y barra lateral.

D-036: actuales/archivadas y páginas anteriores usan exclusivamente proposal_request/FRB. Agrupar por revisión; confirmar archivo con todas las versiones/encargos incluidos, sin resolver borradores. Archivo solo de consulta, sin aplicar/descartar o inventar un cambio externo. Conservar filas/cursor ante fallo, deduplicar y descartar respuestas de otra selección por generación. Bloquear doble escritura/cambio de modo durante archivo, incluso ante actualización de snapshot. Tras cargar anteriores, avisar de cambios sin colapsar la lectura; actualizar explícitamente reinicia páginas. Mantener marcos, acciones fijas, foco/scroll, barra lateral y estilo general.

D-037: mostrar chat_request y respuesta de versión como datos de procedencia, sin IDs técnicos ni permiso inferido. Texto largo con resumen y extracto desplegable, recorte indicado y acciones fijas; no desplazar todos los cambios detrás de la solicitud completa por defecto. Observar executionRevision junto al historial para nuevos alcances/borradores antes de otra respuesta. El envío natural no usa un parser Dart ni exige formulario nuevo. Aplicar conserva API humana; guía no se activa desde el chat. Mantener estilo general y barra lateral.


D-032: avisos en el chat desmarcados al crear; explicar resultado/contexto compartidos con el modelo y destino fijado al configurar. Preservar consentimiento/borrador ante fallo. ChatEntry.execution distingue referencia/estado del intento de una delegación o llamada MCP conversacional. El enlace comprueba instancia/política/revisión/intento/estado y comparte showExecutionRun con la ficha. Nunca reanudar por comprobar. Estado/leyenda legibles, foco y scroll, cargas tardías descartadas, errores recuperables y elección humana sin preselección. Mantener barra y estilo general.

D-042: la ficha de construcción consume solo bot_presentation para resumen/actividad narrativos. No presentar job.summary ni events[].message del helper, ni usarlos como fallback. Validar IDs/secuencias/tamaño/fecha; origen ausente o error usa estados operativos fijos y recuperación visible. Conservar campos técnicos para validación histórica de acciones, sin convertirlos en mensajes. Encolar refresh recibido durante otra lectura para no perder cambios de mensajes de trabajos sin sondeo. Mantener estilo y barra lateral.

D-044: WebAttemptPanel presenta metadatos operativos y páginas del diario por plugin_request, con recarga/error y sin informes libres. Reutilizar McpAttemptReviewDialog para applied/not_applied, sin preselección o reenvío. La función pendiente queda bloqueada; otras independientes siguen disponibles con metadatos completos. FunctionProbe conserva bloqueo recibido antes de recarga, incluso si cambia el borrador, y solo lo libera un recibo correspondiente. No liberar por cerrar diálogo, cambiar ficha o error. Conservar estilo general/barra y validar ventana mínima/texto ampliado.

Si falla/está ocupado el catálogo web, consultar el diario del plugin seleccionado por separado para no perder bloqueos en registros conservados. Fallo del diario marca la ficha no disponible y bloquea el probador; una recarga de catálogo fallida no habilita otra llamada. Esta consulta usa solo SQLite por FRB, sin pedir al navegador que ejecute.

## Detener respuesta — D-047

Consumir TurnProgress y TurnCancellationBackend, sin inferir identidad desde busy. El botón actual del compositor cambia a Detener solo durante Speak; bloquea duplicados mientras espera y mantiene el campo editable. stop_response no devuelve snapshot: no reinstalar estado tardío ni borrar borrador/cita sin ACK. Restituir foco solo si sigue la misma generación o ya terminó. MessageInterruption se muestra como estado semántico dentro de la burbuja, separado del texto literal; añadir «sin guardar» si no hubo commit. Probar claro/oscuro, mínimo, texto ampliado y distancia al degradado del lector. Conservar barra lateral, materiales y tipografía generales; ver [D-047](../decisions/chat-cancellation.md).

## Nombres y atajos — D-048

La ayuda de AppTooltip no es el nombre de la acción. Verificar etiqueta, rol y tap en el nodo del botón efectivo, incluidos espera/deshabilitado; no basta encontrar un contenedor etiquetado. Los campos conservan nombre y valor al editar/limpiar, sin duplicar placeholder ni excluir su semántica editable. Mantener el estilo resuelto del hint al sustituir su widget. CallbackShortcuts del compositor define Enter/Enter numérico y sus variantes Mayús explícitamente; el salto respeta selección UTF-16, coloca cursor y no envía. Separar pruebas de KeyEvent con plataforma macOS de entrega física del modificador; ver [D-048](../decisions/native-accessibility.md).

## Confirmación de cierre nativo — D-049

AppDelegate conserva terminationPending mientras termina una solicitud aprobada. Solo después de requestQuit=true retirar el delegado de la ventana principal y responder a AppKit: window_manager no debe enviar eventos a un motor destruido. Rechazo/error conserva el delegado y libera el pendiente para reintentar. No desconectar eventos al comenzar una solicitud todavía rechazable ni silenciar avisos del motor. Probar esta clase y el canal Dart por separado; después verificar Salir en una instancia real propia. Ver [D-049](../decisions/native-termination.md).

## Revisiones y teclado — D-056

AppReviewDialog consume DismissIntent solo como cancelación libre; busy bloquea Escape/PopScope y el cierre por fondo sigue como lo defina el llamador. FocusScope conserva el alcance del atajo al deshabilitar controles; SingleChildScrollView primary=true asocia Scrollbar en macOS. Tras inspección/refresco de propuestas, restituir el foco solo con misma instancia, nodo unido y fila habilitada; no enfocar un invocador retirado. Probar IO/fallo y cancelación sin resolver. Conservar estilos/medidas/barra existentes. Ver [D-056](../decisions/review-keyboard.md).


## Teclado de revisión MCP

McpAttemptReviewDialog conserva su superficie y añade Actions/DismissIntent con FocusScope propio: Escape cancela solo cuando no guarda, también después de error. PopScope y controles mantienen el bloqueo durante IO. No retirar el bloqueo de la herramienta ni registrar una resolución al cancelar. Verificar foco de retorno al invocador, sin opción inicial o llamada implícita; el recorrido stdio nativo conserva evidencia separada de la prueba con Completer. Ver [evidencia](../verification/native-mcp-probe.md).


## Próxima ejecución en la lista de Encargos

Consumir únicamente AssignmentRecord.nextExecutionAt proyectado por Assignments/FRB; no deducirlo del horario del documento o calcular calendarios en Flutter. Mostrar fecha completa/año/hora local y estado con «Prevista»; ausencia conserva tipo/estado y fecha inválida recibida informa indisponibilidad. Solo el subtítulo previsto permite hasta tres líneas y tinta al 80 %, manteniendo valores por defecto de SidebarTile para las demás filas. SidebarSearch conserva una línea con elipsis y su nombre semántico completo al mínimo/texto ampliado. No cambiar navegación, tema/material o tamaños generales. [Evidencia](../verification/assignment-next-execution.md).


Estados vacíos: ContentEmpty y SidebarEmpty comparten su gris informativo por tema (#a3a3a3 oscuro/#555555 claro); conservar dimensiones, tipografía, navegación y superficies. El mensaje de ausencia no es un control deshabilitado. Validar texto normal con 4,5:1 sobre el fondo probado, incluyendo texto 150 %, sin presentar la captura de una versión previa como prueba nativa del arreglo. [Evidencia](../verification/empty-state-contrast.md).


Editor básico de Encargos: el pie usa Wrap alineado al final, con separación 12/8, para conservar completas Cancelar/Guardar al mínimo y texto 150 %. No compensar el desbordamiento reduciendo fuentes o cambiando la barra. fieldFocus pertenece a AssignmentEditor; asignar un nodo estable a cada campo, solicitarlo en revealField después de desplegar/layout y liberarlo al desmontar. Verificar título y después objetivo vacíos en GennieApp real, no solo AssignmentEditor sin barra.


La etiqueta de cadencia del editor básico es Intervalo de revisión; la unidad y opcionalidad van en helperText con helperMaxLines existente. Con followUpEnabled el campo es requerido y no debe anunciarse opcional. Mantener el mismo labelText al editar y comprobar semántica/RenderParagraph con Inter, mínimo/150 %; no añadir otro contenedor semántico si InputDecorator ya asocia el nombre.


En Pasos de preparación, la instrucción Un paso por línea. pertenece al helperText, manteniendo etiqueta breve y el control multilínea actual. Las etiquetas se miden con el ancho del editor completo (barra y sidebar incluidas), Inter y texto150 %, no con un campo aislado a ancho normal.


AssignmentProfile: rótulos de entrega/zona horaria usan la tinta base al 80 % en claro y 70 % en oscuro. Conservar grupo, fuentes, tamaños, bordes y espaciados. La comprobación de contraste del detalle no cambia automáticamente estilos del editor.


Legibilidad de AppDetailHeading: etiqueta y DefaultTextStyle del subtítulo al 80 % claro/70 % oscuro, sin cambiar disposición o controles. En el tema local de AssignmentEditor, labelStyle/helperStyle usan 80 % claro y conservan 65 %/62 % oscuro. Conservar las demás propiedades del tema y los bordes. [Evidencia y límites](../verification/heading-form-contrast.md).

Textos auxiliares de Encargos: introducción, vacío de aplicaciones, resúmenes plegados y nota de preparación del editor usan tinta 80 % claro/70 % oscuro. AssignmentProfile comparte secondaryInk para rótulos/URL y markerInk 80 % claro/65 % oscuro para números/viñetas; _AssignmentDetailDisclosure conserva 70 % oscuro y usa 80 % claro. No aplicar esta corrección a iconos/bordes ni cambiar disposición/material/barra. [Evidencia](../verification/assignment-secondary-contrast.md).

El focusedBorder de AssignmentEditor conserva grosor/geometría y usa tinta 65 % claro/50 % oscuro. Su contraste no textual se comprueba por separado del texto y del borde inactivo; no modificar campos o fondos para corregirlo. [Evidencia](../verification/assignment-focus-contrast.md).

SidebarSearch: focusedBorder y el icono Limpiar usan tinta 65 % en claro, conservando 50 %/55 % oscuro. La lupa conserva su estilo; no cambiar forma, tamaño, radio, grosor, padding, material, semántica/callback o barra vertical por este ajuste. [Evidencia](../verification/sidebar-control-contrast.md).

# D-047 — detener la respuesta y conservar su fragmento

Estado: implementado localmente el 29 de octubre de 2026; continúa el goal. Sin commit/push, instalación o despliegue. Continúa [D-045](transport-cancellation.md) y el [plan de cancelación](../plans/chat-cancellation.md).

## Identidad y cierre

Cada entrada escrita o dictada reserva una identidad opaca de Speak y un CallContext propio. Controller compara esa identidad antes de cancelar: una petición vencida es inocua para la siguiente respuesta, otros trabajadores y avisos nuevos. No reutiliza la bandera permanente de cierre del módulo. Connect, nuevo chat y ajustes no se presentan como respuestas cancelables. Todos los eventos conversacionales llevan la identidad; bootstrap descarta eventos ajenos y conserva la reserva visual hasta consumir Finished.

Un gate corto ordena observación, escritura y ACK frente a Detener; nunca contiene red. El contexto cubre conexión, handshake e IO de la respuesta. Cancelar termina y recoge el app-server propio, retira su sesión y, mediante EOF/D-045, cancela las herramientas contextuales pendientes. El siguiente envío explícito vuelve a conectar con el hilo persistido; no reenvía la entrada anterior ni inicia un reintento automático. La prueba del worker usa un proveedor ficticio; la reanudación con una cuenta real sigue pendiente.

El coordinador vacía narración pendiente e interrumpe solo su reproducción. No narra fragmentos terminales ni acepta deltas/ACK de otra generación. Una cancelación antigua tampoco borra el error o confirmación de una operación posterior. Una acción remota puede haber ocurrido: se conservan los diarios inciertos, sin reconciliar ni autorizar repetir. Cancelar el runtime privado compartido de Browser puede cerrar todas sus pestañas, conforme a D-045.

## Persistencia

Presence 0019 añade interruption opcional con valores cancelled/failed, limitado a respuestas del asistente correlacionadas con un turno. Las filas anteriores quedan con NULL. Las 54 migraciones anteriores del workspace se conservaron byte a byte respecto al archivo sellado del piloto.

Se sella una sola vez únicamente el fragmento realmente observado al terminar por cancelación o fallo; no se escribe cada delta ni audio. Mensaje, estado, asociación a herramientas, actividad y cierre incierto de herramientas pendientes comparten transacción. Un ítem completo anterior sigue completo. La identidad de turno/ítem impide duplicados, también cuando no existe itemId; un conflicto no degrada un mensaje completo. Se exige la entrada de origen y un destino visible: una respuesta tardía no resucita datos eliminados.

El aviso de guardado sale después del commit. Si falla, el fragmento permanece visible como «sin guardar», incluso si su texto coincide con un mensaje antiguo; el fallo de finalización de herramientas también se comunica. Salir cancela y une el worker antes de cerrar SQLite. Las pruebas comprueban rollback inducido, reapertura, FK, atribución y migración 0018→0019 con filas anteriores intactas, sin usar datos personales.

## Interfaz

El botón del compositor conserva su ubicación y geometría y cambia de Enviar a «Detener respuesta» solo durante Speak. Mientras espera muestra «Deteniendo respuesta…» y bloquea pulsaciones duplicadas. El campo sigue editable; se conservan borrador/cita sin ACK y se restituye foco sin actuar sobre otro turno. FRB stop_response devuelve únicamente la aceptación de la petición: Dart no instala un snapshot antiguo al completarse esa llamada.

La burbuja existente muestra «Respuesta detenida» o «Respuesta interrumpida» como estado semántico separado del contenido, con «sin guardar» cuando corresponde. Se conserva el texto literal para lectura y citas. El rótulo queda dentro de la burbuja para evitar el degradado inferior del lector. No se cambian barra lateral, navegación, vidrio o tipografía general.

## Evidencia y límites

- Workspace Rust: 401 pruebas correctas y 27 optativas omitidas. Incluye worker real con cola/IPC y procesos shell propios, cancelación exacta/repetida/tardía, siguiente entrada, cierre con respuesta pendiente y fallo del proveedor después de un delta. La inyección del proceso ficticio solo existe bajo cfg(test).
- Flutter: 343 pruebas correctas y 49 optativas omitidas; quince casos nuevos de cancelación. Teclado, semántica, foco, doble clic, error recuperable, borrador/cita y respuestas tardías cubiertos con widgets.
- Dieciocho capturas software: activo, deteniendo y guardado; claro/oscuro; 1100 × 720, 640 × 420 y texto 150 %. Inspección y límites geométricos verifican el rótulo fuera del degradado inferior. Contraste del estado nuevo entre 7,12:1 y 7,93:1, medido sobre las capturas; no certifica material nativo.
- Sonda optativa scripts/probes/chat_cancellation_bridge.py: dos procesos Flutter/FRB y bases SQLite privadas, proveedor apagado. Lee ambos estados persistidos, preserva un error nuevo frente a Detener vencido y verifica integridad/FK y cero hilos, trabajos, llamadas MCP o ejecuciones. Cierra backend y retira fixtures. No prueba cancelar un proveedor activo desde una ventana nativa.

Evidencia en .verification/assistant-chat-real/stop-{workspace-final2,clippy-final,flutter,analyze-final,widgets4,bridge-final,debug-final,invariants-final2}.log, stop-migrations-final.json, stop-finish.json y stop-{active,pending,saved,contrast}-*. El primer test adicional de fallo del proveedor recibió un JSON ficticio mal formado; se corrigió el fixture y se repitió la suite. No se consumió inferencia adicional: los ocho turnos autorizados del piloto D-038 ya se habían utilizado.

Los lanzadores ./scripts/dev.sh --build-only y ./scripts/build.sh completaron biblioteca/helper debug y paquete macOS release. Firma local ad hoc profunda/estricta correcta; logs stop-{release-final,signature-final}.log. No se abrió, instaló o desplegó ese paquete. Los procesos y bases temporales de las sondas se cerraron/retiraron y se conservó la instancia Debug anterior del usuario.

Continúan la interacción física/VoiceOver, la matriz completa de accesibilidad/material, la cuenta real después de Detener, revisión humana del piloto y restantes requisitos del plan principal. La conexión/handshake reciben el contexto por código y sus primitivas tienen regresiones de cancelación; no se acredita aquí una cancelación durante autenticación con cuenta real. Esta etapa no acredita esas comprobaciones ni completa el goal.

Actualización posterior: la [sonda de proveedor](../verification/provider-chat-probe.md) verificó una cita explícita y aislamiento de sesión nueva con cuenta real. Su tercera fase terminó sin solicitar corte; Detener quedó inconcluso y no se ejecutó la reanudación. No sustituye las pruebas FRB/Presence ni acredita cancelación real. La campaña cerró con tres turnos/cero guía, sin repetir o iniciar la comparación condicionada. Una regresión posterior mejora únicamente las métricas del observador para no perder la distinción entre respuesta corta, final sin deltas y carrera de finalización; sin otra consulta a modelos.

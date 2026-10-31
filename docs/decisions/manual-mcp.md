# D-039 — Prueba manual MCP sin iniciar una conversación

29 de octubre de 2026. Implementado localmente. Continúa D-028 y su verificación integral por Flutter/FRB, conservando el estilo general y la barra lateral.

## Problema y cambio

Guardar una conexión funcionaba con Plugins activo y LLM apagado, pero «Descubrir herramientas» fallaba con «Conversación no iniciada». El cliente MCP es un transporte sin inferencia; el fallo procedía del callback de permisos de bootstrap, que exigía obtener el controlador conversacional activo antes de leer la configuración.

Bootstrap ahora consulta el perfil persistido del bot mediante `architects::runtime_settings`. No usa defaults ante errores ni inicia el módulo LLM. El cliente sigue exigiendo comandos/control de aplicaciones y, para programas stdio, también lectura/escritura; se comprueban permisos y revisión antes de crear el cliente o enviar. El constructor conserva el requisito de LLM activo y su perfil de helper intersectado con el bot. No cambian esquema SQLite, FRB, herramientas, autorizaciones remotas ni UI.

## Evidencia

La regresión de composición `manual_mcp_reads_saved_bot_permissions_without_starting_conversation` falló antes del arreglo con el mismo mensaje. Después comprueba el rechazo por permisos reales, su actualización persistida y ausencia de conversación, intentos o trabajos, sin conectar servicios. La sonda optativa `python3 scripts/probes/mcp_lifecycle_bridge.py` usa el escritorio Flutter real, la biblioteca/helper recompilados, SQLite propio y un servidor stdio ficticio con contador sin argumentos. Codex transporta MCP sin cuenta enlazada ni turnos de modelo.

Recorrido por controles de la interfaz: Nuevo plugin→conexión stdio→guardar→descubrir→habilitar/elegir herramienta→guardar permisos→rechazar campo requerido vacío→ejecutar una consulta ficticia. Guardar, descubrir y conceder permisos no envían herramientas. La siguiente llamada provoca pérdida de transporte después de registrar su recepción. Se conserva incertidumbre; una tercera petición con otros argumentos y revisión vigente queda bloqueada. La comprobación humana registra «applied», no ejecuta ni reenvía. Cierre y reapertura conservan la revisión y dejan el cliente desconectado.

Resultado: exactamente dos envíos, un intento completed y otro reviewed/applied, cero trabajos, mensajes o hilos Codex persistidos. Integridad/FK correctas, sin cuerpo de resultado en SQLite. El script retira su DB y solo los perfiles de IDs nuevos de su fixture, conservando los directorios preexistentes. No usa servicios externos, cuentas, modelos, navegador ni encargos.

Tres capturas software inspeccionadas en `.verification/assistant-chat-real/mcp-lifecycle-*`: resultado correcto oscuro a 1100 × 720, pendiente claro a 640 × 420 y revisión clara al mínimo. Se mantiene el marco existente con título/acciones visibles y cuerpo desplazable; no certifican composición macOS o toda la matriz de accesibilidad. La primera captura se reposicionó y volvió a generar para mostrar el estado de resultado fuera del borde del scroll, sin cambiar el producto. La sonda pasó en unos tres segundos; las esperas del harness observan controles habilitados después del refresco real, no solo su presencia.

Cierre automatizado: 365 pruebas Rust correctas y 23 optativas omitidas; 26 widgets MCP correctos, una sonda optativa omitida en esa ejecución y el recorrido real ejecutado por separado. Clippy workspace/todos los targets con `-D warnings`, análisis Flutter, formato, invariantes y diff correctos. Biblioteca/helper debug y paquete release macOS compilados; firma ad hoc local comprobada. No se abrió o instaló el paquete nuevo ni se modificó FRB. Logs `mcp-permissions-{before,after}.log` y `mcp-lifecycle-{bridge-final,workspace,clippy,widgets,analyze-final,debug,release}.log` en `.verification/assistant-chat-real/`. Los fallos iniciales del recorrido y diagnóstico se conservan.

## Comprobación nativa y límites

`scripts/probes/native_ui.py` abrió una instancia release propia durante un máximo de sesenta segundos, con dos mensajes y cuatro encargos ficticios, solo Presence/Assignments activos. El cierre Quit dirigido a su PID terminó con salida 0; SQLite quedó íntegro, sin FK inválidas, planes en revisión 1 y cero trabajos/ejecuciones. El fixture se retiró. No se tocó la instancia Debug que ya estaba abierta ni la base personal. Este arranque usó el paquete anterior al arreglo de permisos y no se presenta como ejecución del arreglo nuevo.

En ese ensayo histórico, Orca enumeró la ventana propia, pero `get-app-state --restore-window` devolvió `permission_denied`: no consiguió una ventana AX. Su consulta separada informa Accessibility/Screen Recording concedidos y capacidades sin enfoque/redimensionado. No se cambiaron permisos del sistema, pulsaron controles o validaron VoiceOver/material físicos. El Orca auxiliar abierto para la comprobación y Gennie propio terminaron; la instancia previa permaneció abierta. Evidencia en `.verification/native-ui/be64672e-03a8-4979-9aa5-6fc67c6ff53c/`.

## Continuaciones verificadas y pendientes vigentes

D-042 y la [auditoría de comunicación](../verification/architect-communication.md) verifican la mediación de rutas conocidas; D-043 conserva contenido MCP nativo; D-044 extiende el diario a web y D-045 aporta cancelación por consumidor/EOF. D-047 conserva identidad y fragmento al detener el chat. Son ámbitos locales comprobados, no una promesa de deshacer efectos o detener facturación remota. No se exige crear una equivalencia universal entre herramientas; falta la muestra semántica de destinatario ambiguo/ruta alternativa, preparada sin nueva autorización live.

D-040/D-054 verificaron copia histórica de 57 sesiones/1.046 mensajes, rollback/restauración y páginas de hasta tres sesiones. El run 11c53fcd acreditó cita y aislamiento de sesión nueva; Detener quedó inconcluso, por lo que no se presentó reanudación real como comprobada. La campaña nueva separa esas etapas de la comparación de Encargos. La [matriz nativa ampliada](../verification/native-scaled-accessibility.md) ya usa Computer Use/AX con otra herramienta y documenta su alcance, sin convertir la limitación histórica de Orca en un bloqueo actual general. Continúan comparación/revisión humana, Detener real y el resto de matriz nativa/VoiceOver. No declarar completo el goal ni certificar toda la etapa de Plugins con estas sondas.


El recorrido nativo posterior 0cde5d70 de [editor MCP](../verification/native-mcp-editor.md) verifica otro ámbito: Plugins activo, Browser/conversación apagados, fuente HTTP ficticia precargada, edición/guardado y recarga por botones nombrados, sin descubrimiento/ejecución. Terminó con Salir 0, revisión 2 deshabilitada, catálogo/permisos vacíos, cero intentos y datos ficticios conservados. El recorrido manual de transporte de D-039 sigue siendo su evidencia propia; el editor nativo no acredita conexiones ni VoiceOver.


## Recorrido nativo del probador y reapertura

La [prueba nativa stdio](../verification/native-mcp-probe.md), preparación 1da742f0, recorrió el release habitual en dos procesos sobre la misma base ficticia, con Plugins activo y conversación/navegador apagados. Descubrir, guardar permisos y rechazar el vacío no enviaron la herramienta. Dos llamadas: una confirmada y otra con salida del servidor tras registrar el efecto privado; interfaz bloqueada y diario uncertain, Salir 0. Al reabrir, el pendiente permanecía; Escape cerró sin resolver y la revisión applied se registró tras comprobar el contador, sin tercera llamada. Segundo Salir 0; dos mensajes/perfil/Encargos intactos, cero trabajos/ejecuciones/hilos de conversación, integridad y limpieza correctas. Perfil MCP nuevo sin auth.json y retirado.

Se reprodujo y corrigió también Escape en McpAttemptReviewDialog mediante Actions/FocusScope sin cambiar estilos. 376 Flutter/49 optativas omitidas, análisis y release/firma correctos; seis capturas de revisión idénticas. La protección de guardado/error se prueba con un Completer; el recorrido nativo acredita cancelación libre y revisión real en SQLite sobre efectos ficticios. No sustituye VoiceOver, teclado con modificadores físicos, matriz nativa mínima/ampliada ni servicios externos. Campaña de modelos aún sellada y pendiente.

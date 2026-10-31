# Historial nativo de 300 sesiones y cita distante

29 de octubre de 2026. Preparación `.verification/native-large-history/ea626a4b-a527-46c7-a91d-10df939467c4`, release habitual 1100 × 720 con SHA-256 de origen 074eec394111fac8b38d4abd330acc21d7db4efb3cecc0479e92a257c73b0660. Copia firmada con bundle UUID propio, sin cambios de Dart/Swift/tema/MediaQuery o preferencias del sistema. No cambia el código del producto, el estilo o la barra; no requiere otra suite/build.

## Fixture y alcance

SQLite privada con 300 sesiones de Gennie, dos mensajes por sesión, 600 mensajes en total. Fechas separadas, identificadores de proveedor ficticios y sesión activa 300. La respuesta 600 cita el mensaje 1 de la primera sesión. Conversación, guía, navegador, audio y demás módulos de efectos apagados; solo Presence/Assignments activos. El preflight comprueba integridad/FK, asociaciones, cita, cero hilos Codex y retira su base antes del recorrido gráfico.

Los hashes de todas las sesiones/mensajes y la referencia de cita se comparan antes/después. No se usan datos personales ni cuentas. El contador de consultas del lector no se instrumenta: la prueba no atribuye una cantidad de llamadas o tiempo de carga a lo observado. El máximo de tres sesiones por página conserva su evidencia Rust/FRB de D-054; este ensayo añade interacción real del renderer.

## Recorrido

PID 70520. Inicialmente, AX describió las sesiones 298, 299 y 300 y el control Cargar mensajes anteriores; la sidebar mantuvo un único asistente Gennie. Al desplazar hacia arriba aparecieron mensajes de las sesiones 296/297; volver abajo conservó la respuesta reciente y su cita.

Pulsar el botón nombrado Ver tu mensaje original localizó el mensaje de la sesión 1. Se observaron Inicio del historial, su fecha/separador y contorno de foco. La tecla Menu abrió Responder sobre ese original; Down/Return preparó en el compositor su fragmento exacto. Se canceló desde Cancelar respuesta sin enviar. Una lectura ro en ese punto confirmó sesión activa 300, 600 mensajes y cero hilos del proveedor. No se reactivó la sesión histórica ni se creó otra.

Salir mediante el menú propio terminó con código 0, antes de recoger el hijo. Capturas y árboles permanecen en la traza de Computer Use, incluidas carga previa, original con foco y previsualización cancelable. No se interpreta el input sintetizado como prueba de modificadores físicos.

## Cierre y límites

run/manifest.json confirma hashes idénticos de las 300 sesiones y 600 mensajes, cita 600→1, 300 asociaciones al asistente, sesión activa 300 y cero hilos Codex. Las revisiones mínima/máxima de Encargos siguen en 1/1. Cero trabajos/ejecuciones; integridad/FK y canales de motor correctos. Fixture y preferencias del bundle QA retirados, Debug 49455 conservado, release intacto. Fuentes owning, fixture y driver archivados; sello de campaña de modelos coincidente y cero turnos/guía.

El resultado acredita esta carga y navegación nativas con datos ficticios, no un benchmark de volúmenes arbitrarios, conteo independiente de consultas, VoiceOver, contraste nativo cuantitativo o toda la matriz mínima/ampliada. Las autorizaciones de modelos y cambios reales de accesibilidad siguen pendientes. Goal activo, sin commit/push, instalación o despliegue.

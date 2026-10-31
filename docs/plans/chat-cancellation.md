# Detener una respuesta desde el chat

Implementado localmente el 29 de octubre de 2026 como [D-047](../decisions/chat-cancellation.md), después de D-045 y D-046. Este documento conserva el diseño y sus criterios; el estado comprobado y los límites están en D-047. Se conserva el goal completo y el estilo general/barra lateral. Sin commit/push ni despliegue.

## Problema anterior y diseño aplicado

Antes de D-047, Controller de LLM serializaba connect/speak/nuevo chat/ajustes con pending, sin identificar cada Speak para cancelarlo. La bandera ProcessHandle.cancel pertenece al cierre del módulo y no debe reutilizarse para envenenar todos los turnos posteriores. D-045 sí permite cancelar las llamadas MCP/Browser cuando termina el proceso consumidor. El coordinador recibía deltas/finales sin ID local de operación y no tenía un control Detener. Presence guardaba mensajes finalizados; un delta no persistido podía perderse al interrumpir.

La implementación añade una identidad opaca por operación conversacional y un contexto cancelable propio. El control compara esa identidad antes de cancelar; una petición tardía no toca otra respuesta, el constructor, la guía o llamadas manuales. La siguiente entrada explícita solicita reanudar el hilo persistido, sin reenviar la anterior, reintentar automáticamente o crear otro contexto silencioso. Connect/nuevo chat/guardar ajustes no se presentan como una respuesta cancelable. La comprobación con cuenta real sigue pendiente.

Un gate corto serializa observación/commit/ack frente a la cancelación; nunca se sostiene durante red. Si un ítem ya se confirmó antes de detener, sigue completo. Un delta/evento posterior al cierre de esa generación no se guarda, narra o atribuye a una respuesta nueva. Todos los eventos del turno deben transportar su identidad; el coordinador filtra la identidad activa y conserva orden. Dictado y texto usan la misma entrada del coordinador para no perder esa identidad.

## Interrupción y persistencia

Detener cancela únicamente el proceso conversacional propio y su contexto; D-045 propaga EOF a herramientas. Una acción remota ya enviada puede haberse aplicado; los diarios conservan sus pendientes y ninguna cancelación los reconcilia. Gennie Web puede cerrar las pestañas de su runtime privado al abortarlo. Vaciar narración pendiente e interrumpir únicamente TTS propio; el texto interrumpido no inicia narración nueva.

La regla de no guardar parciales cambia solo para un mensaje terminado por interrupción: guardar una vez el texto realmente recibido con atribución/estado explícito, sin prefijos añadidos o texto inventado. No persistir cada delta ni audio. Presence 0019 expresa cancelado/fallido, conservando mensajes/migraciones anteriores. Mensaje, estado y asociación de herramientas se confirman juntos; la finalización mantiene desconocidas las llamadas no confirmadas. No duplicar ítems completos al guardar el fragmento en curso.

Fallo de persistencia conserva el texto visible y comunica que no se guardó; no declarar éxito por emitir un evento. Confirmar el estado después del commit. Pruebas de reinicio, FK/atribución exactas, borrado y respuesta tardía deben probarlo en SQLite propia. Salir une el worker y sus escrituras antes de cerrar DB.

## API y presentación

Exponer por FRB la identidad/estado cancelable opcional y una operación que solicita detener esa identidad. El cliente no sustituye un snapshot nuevo por la respuesta tardía de la petición de detener. Preservar borrador/cita si la entrada no recibió ACK, y no borrar la edición nueva cuando llegue un ACK anterior.

En el compositor, el mismo botón y geometría sustituyen el indicador ocupado por Detener respuesta solo durante Speak. Mientras solicita/cierra: Deteniendo… accesible, sin doble envío; el campo conserva edición. Al terminar, volver a Enviar y restituir foco sin afectar otra generación. Un mensaje persistido interrumpido usa la burbuja actual y un rótulo de estado separado del texto. No cambiar navegación lateral, marco, vidrio o tipografía general; no revelar IDs técnicos.

## Verificación exigida

- LLM: cancelar antes de enviar, durante handshake, después de deltas y entre ítems; cancelación repetida/tardía, siguiente entrada y cierre del módulo. Mensaje completo anterior sigue completo; título opcional no arranca después de cancelar.
- Coordinador: generación exacta para texto/dictado, eventos tardíos descartados y ningún borrador nuevo borrado por ACK previo. Error de guardado conserva fragmento; narración pendiente se retira.
- Presence: fragmento/estado/herramientas atómicos, ID de ítem sin duplicados, conservación al reabrir, FK/borrado y rollback. Migraciones previas intactas; ninguna escritura sobre DB personal para probarlo.
- Flutter: botón/estado, doble clic, error recuperable, teclado/foco/semántica y borrador/cita conservados; ninguna petición con ID de otro turno.
- Visual: claro/oscuro, 1100 × 720 y 640 × 420, texto 150 %, movimiento reducido y estados con texto largo. Inspeccionar capturas y contraste de rótulos nuevos. Separar esto de material/VoiceOver físicos.
- Integración: proveedor/efectos ficticios y FRB/SQLite reales propios, sin consumir nuevos turnos del piloto agotado. Comprobar proceso propio terminado y conservación de la instancia del usuario. Anunciar procesos/puertos antes de iniciarlos.

No marcar el goal completo al cerrar esta función: mantiene los demás requisitos y las verificaciones pendientes del plan principal.

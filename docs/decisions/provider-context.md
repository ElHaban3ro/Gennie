# D-033 — Avisos locales en la sesión principal del bot

Estado: implementado localmente el 29 de octubre de 2026; verificación al final del documento. Continúa D-029/D-032 sin alterar el estilo ni la navegación. El goal general permanece activo.

## Problema y alcance

Un aviso redactado por el bot y guardado en Presence no formaba parte del hilo remoto principal. Al preguntar después por ese resultado, el bot podía carecer del mensaje que el usuario acababa de leer. Ahora el siguiente envío explícito del usuario incorpora primero los avisos atribuidos de esa misma conversación, proveedor/hilo y época. Leer el historial continuo, cambiar su selección o recibir un aviso no inicia inferencia ni reanuda otra sesión.

Solo son elegibles los mensajes assistant con referencia de constructor o ejecución. Las respuestas ordinarias, errores system y mensajes históricos de otras sesiones no se copian. Una cita explícita conserva su contrato anterior: fragmento limitado, sin reactivar ni conceder acceso al hilo original. Los avisos siguen procediendo de la mediación del bot, nunca directamente del helper o de la guía.

## Propietarios y transacción

LLM define `context::PreparedUser`, mensajes y referencias tipadas; Presence implementa el diario. No hay dependencia inversa, SQL entre apps, app nueva, HTTP ni worker adicional. `prepare_user` valida el destino activo y confirma juntos entrada literal, cita opcional, reserva de contexto y asignación de mensajes. La revisión de historial cambia después del commit; un fallo impide el envío al proveedor.

Presence 0018 añade recibos `reserved/accepted/uncertain` y una cola de elegibilidad por mensaje, sin duplicar cuerpos. La entrega registra esa elegibilidad junto al mensaje/atribución/recibo del inbox. La migración llena la cola a partir de atribuciones existentes, sin reescribir IDs, textos, fechas o sesiones, y conserva las migraciones anteriores. Los índices evitan recorrer mensajes ordinarios en cada entrada; contar/retirar un atraso puede recorrer sus metadatos pendientes, sin cargar todos sus cuerpos.

El lote toma hasta doce avisos recientes y doce referencias inciertas, con un presupuesto compartido de 8.000 caracteres de valores y límite de 64.000 bytes codificados. Las referencias y fechas cuentan en el presupuesto; el framing JSON y los textos fijos no. El texto individual se limita a 1.600 caracteres. Los avisos se presentan cronológicamente y se explicita cuántos quedaron omitidos. Todo el conjunto pendiente se asigna una sola vez al recibo: no se vuelca el atraso en turnos sucesivos ni se presenta como leído lo omitido.

## Protocolo y fallos

El adaptador usa `thread/inject_items` con mensajes Responses assistant. Conserva el texto ya publicado por el bot y añade metadatos separados de Gennie como datos históricos sin autorización nueva. Después de la respuesta del proveedor confirma el recibo local; solo entonces inicia `turn/start` con el texto literal del usuario, o con el contrato de cita anterior. Incorporar contexto no crea por sí mismo un turno de usuario ni generación. No se cambia el título, la selección activa ni la entrada visible para envolver los avisos.

Si falla el envío, la respuesta o el commit del recibo, no se inicia el turno ni se reenvían automáticamente los cuerpos. Una reserva al arrancar, o encontrada por el siguiente envío del worker serial, pasa a incierta. El siguiente envío explícito puede incorporar una nota interna acotada con referencias/conteos, sin el cuerpo anterior y sin instruir repetir efectos. Confirmar esa nota reconoce el aviso de incertidumbre, sin convertir el recibo original en aceptado. Si falla también ese aviso, podrá volver a indicarse la incertidumbre en otro envío explícito. No se promete recepción exactamente una vez ante un corte entre sistemas.

El cierre del hijo y su recuperación siguen el lifecycle existente. El borrado local elimina recibos y elegibilidad por FK e invalida la época; un ACK tardío no confirma otra sesión. Una llegada posterior a la reserva permanece para la próxima entrada. Un CLI sin soporte del método produce un error visible; no hay sustitución silenciosa por un prompt de usuario ni reenvío del cuerpo.

## Consulta de resultados

`assignments_execution_get` permite al bot leer un run por su referencia cuando la política original compartía resultados con esa misma conversación/época. El origen lo inyecta bootstrap, no lo acepta del modelo. Devuelve el resultado acotado y sus metadatos, conserva incertidumbre/resolución y omite argumentos originales. No ejecuta, activa, pausa o reconcilia; rechaza otra sesión, otra época y políticas sin consentimiento. Las observaciones siguen siendo datos no confiables. Una cita o ID adivinado no amplía el permiso. Los trabajos del constructor conservan su consulta existente.

## Evidencia y límites

Pruebas SQLite: aislamiento, ambas atribuciones, entrada/cita/reserva atómicas, fallos de elegibilidad y ACK, límites compartidos, atraso omitido, llegadas tardías, cambio de sesión/época, borrado, migración 0017→0018, FK y recuperación desde archivo. Transporte simulado: orden inject→ACK→turn, entrada literal intacta, ausencia de RPC adicional sin pendientes y rechazo antes del turno al fallar proveedor o recibo. Consulta MCP: permisos por origen, ausencia de argumentos y ningún efecto adicional.

`python3 scripts/probes/provider_context_protocol.py` confirmó el método con el Codex instalado en perfil/directorio temporales, sin credenciales ni turnos de generación. Los dos bloques de texto y sus datos quedaron en el rollout propio, retirado al cerrar. Es evidencia del protocolo y persistencia; no mide comprensión del modelo ni calidad semántica. Se contrastó con la [documentación oficial del app-server](https://learn.chatgpt.com/docs/app-server).

Registros y cifras finales en testing y el plan. No se modifica Dart ni la API FRB: se conservan las capturas de D-032 como evidencia de esa interfaz, sin presentarlas como pruebas nuevas de este protocolo. D-034 continúa la guía contextual consentida; evaluación empírica, copia histórica y matriz física/VoiceOver siguen pendientes. Sin commit, push, instalación o despliegue.

# D-034 — Valoración contextual de avisos

Estado: integración local del 29 de octubre de 2026, con verificación al final y en testing. Continúa D-029/D-031/D-032/D-033; no demuestra superioridad semántica ni completa el goal general.

## Acuerdo y alcance

La entrega programada y la preparación interna siguen separadas. Una configuración de preparación puede autorizar `contextual_notice`, desmarcado por defecto: necesita criterios de seguimiento del encargo, destino de chat comprobado y guía remota disponible. Las configuraciones anteriores se interpretan como false. La política conserva el acuerdo original por revisión; habilitar el arquitecto guía por sí solo no crea políticas, llamadas ni observadores.

Las observaciones exitosas de esa preparación atraviesan guía→bot. La guía valora registrar, comunicar, preguntar u ofrecer ayuda mediante cuatro probabilidades acotadas. Son consejo para el bot: no se suman como categorías excluyentes, no hay corte universal ni selección automática por el mayor número. Gennie decide con el acuerdo, observación, conversación y valoraciones anteriores. Una pregunta u oferta solo produce texto; no concede autorización, modifica el plan, construye plugins ni ejecuta otra función.

Los resultados fallidos, cancelados, omitidos e inciertos siguen la vía de hechos de D-032, sin filtrarse por una clasificación de éxito. En particular, la incertidumbre de un efecto requiere comprobación; una guía no puede convertirla en éxito o permiso de repetir. Una entrega fijada por horario continúa su contrato anterior y no se silencia por este control.

## Propietarios y consentimiento

Assignments conserva el booleano y copia objetivo/criterios de la versión original al outbox. No añade tablas ni cambia sus migraciones: los nuevos campos del documento tienen defaults conservadores. Bootstrap adapta a los tipos de LLM; ninguno consulta SQL de otro propietario. Los mensajes anteriores sin guidance conservan su serialización para reconocer recibos existentes.

`contextual_notice_permitted` pertenece a Assignments y comprueba intento exitoso, origen completo, consentimiento original, definición vigente del encargo y equivalencia entre configuración original y actual. Un cambio de alcance/configuración invalida valoraciones pendientes. Pausar con una copia idéntica permite terminar resultados ya iniciados, como D-032. La comprobación se inyecta en la preparación de guía y vuelve a ejecutarse antes del envío, antes de llamar al bot y al recibir su salida. No se mantiene un lock SQLite durante la red. No se retracta una solicitud que ya superó su comprobación e inició el envío.

La acción humana `revoke_guide`, independiente del plugin y del estado del ejecutor, guarda una nueva versión con contextual_notice=false y delivery=None. No requiere catálogo, acceso a la fuente o guía disponibles. Mantiene calendario, parámetros y autorización de la función; las observaciones futuras quedan en Actividad. Invalida nuevas valoraciones de resultados pendientes de la versión anterior; una consulta en curso puede terminar, sin iniciar el siguiente modelo o entregar su mensaje si el permiso ya no está vigente al comprobarlo. Los hechos operativos ya iniciados conservan el tratamiento explícito de incertidumbre. El modelo no dispone de esta acción humana.

Las salidas ya preparadas revalidan también el permiso antes de entregar, incluso al reabrir. Esta consulta no necesita preparar un modelo ni tener la guía remota disponible. Un permiso retirado convierte el pendiente en registro sin mensaje; un fallo de lectura conserva el texto listo para comprobarlo después, sin otra inferencia. La validación queda fuera de la transacción de entrega para no reentrar el mutex compartido de SQLite.

## Configuración e inferencia

Proactivity sigue siendo cliente sin scheduler, herramientas ni observadores. `prepare` fija configuración/revisión sin red; `PreparedEvaluation` expone únicamente modelo/revisión y permite una evaluación consumiendo esa preparación. Antes del envío comprueba módulo/perfil habilitados y configuración todavía coincidente; compone la personalización fijada. No expone claves, no implementa Debug de secretos ni introduce variables.

LLM recibe una fábrica configurada por bootstrap, no importa Proactivity ni Assignments. Su dependencia opcional de lifecycle con proactivity lo inicia antes del worker del bot cuando está activo y lo detiene después; evita interpretar su arranque como indisponibilidad. Proactivity no depende de iniciar LLM: su arranque solo habilita el cliente y todas las migraciones ya terminaron. No se fuerza la activación de ese módulo.

La guía recibe criterios del encargo, observación de hasta 8.000 caracteres, los doce últimos mensajes de la sesión de origen con presupuesto de 8.000 y hasta tres valoraciones consentidas anteriores de la misma política/revisión/origen. Ese historial serializado tiene presupuesto de 8.000 caracteres y observaciones de hasta 2.000, con recorte explícito. No es un historial completo, ni incorpora otras sesiones, versiones o resultados sin consentimiento; conserva únicamente los fragmentos que el usuario citó explícitamente en esa conversación, según D-029/D-033. El estado codificado tiene límite de 256.000 bytes. No se comparten argumentos originales de la función ni credenciales del host. Los textos son datos no confiables, sin elevarlos a instrucciones.

Gennie recibe ese estado y las probabilidades validadas, con su perfil real y capacidades vacías. Devuelve acción cerrada, motivo de hasta 800 caracteres y mensaje de hasta 1.200; record exige ausencia de mensaje. Las demás acciones pasan por la entrega transaccional e identidad/época de Presence. La guía nunca escribe al historial o habla al usuario. Si se entrega, D-033 incorpora posteriormente ese mensaje del bot al contexto principal de su sesión.

El transporte selecciona explícitamente `ContextualDecision` con esquema cerrado de `action/reason/message`, todos requeridos y mensaje nullable; no reutiliza el esquema `Message` de los avisos ordinarios/delegaciones. El parser sigue validando relaciones y límites. La [regresión del transporte](../verification/contextual-notice-contract.md), añadida el 30 de octubre, reprodujo y corrigió que el esquema anterior solo admitiera message y provocara fallo conservador en cualquier decisión contextual válida. Las pruebas anteriores con respuestas inyectadas no cubrían esa incompatibilidad.

## Persistencia y fallos

LLM 0009 añade configuración/estado/duración/recibo de guía, consejo validado, acción y motivo al inbox existente. Conserva 0001..0008 y sus documentos/recibos. Guarda solo probabilidades y uso/coste numéricos reportados, sin inventar métricas ni persistir errores libres. La configuración identifica el modelo solicitado y la revisión capturada; no afirma verificar el modelo interno del proveedor.

Reserva antes de cada llamada y confirma consejo antes de preparar al bot. Solo un intento de guía y uno de bot por resultado. Un fallo de reserva impide esa llamada; pérdida posterior o cierre no repiten consultas. Las reservas interrumpidas se conservan como resultado sin aviso contextual, con causa fija. Una salida lista se entrega idempotentemente sin otra inferencia. SQL discarded distingue mediante notice_action=record la decisión de guardar sin mensaje del descarte de un destino eliminado.

Ante guía no disponible, salida inválida o valoración interrumpida/fallida se guarda el resultado consultable en Actividad y no se improvisa un aviso contextual. Las siguientes ocurrencias autorizadas son observaciones nuevas; no se vuelve a valorar automáticamente el mismo intento. El bot no sustituye ni oculta el resultado del ejecutor. Los errores de almacenamiento del worker usan el canal existente sin publicar errores privados.

Al cerrar no se inician más fases; se cancela el hijo Codex propio y se une el worker antes de SQLite. Una solicitud HTTP de guía ya enviada conserva el timeout existente de 30 segundos y puede demorar ese cierre; no se promete cancelación inmediata de reqwest. No hay proceso nuevo, listener HTTP ni rutina de modelo al arrancar sin resultados consentidos pendientes.

## Interfaz y evidencia

Seguimiento del 30 de octubre: [cuatro acuerdos ficticios con guía/bot reales](../verification/contextual-notice-probe.md), después del protocolo sin cuenta y la corrección del esquema. Cuatro decisiones esperadas, tres entregas únicas al origen y ninguna repetición al reabrir, sin efectos o cambios de planes. Coste completo desconocido y revisión asistida separada de la única elección humana anterior. El resultado no demuestra utilidad general; los ensayos inferiores conservan su alcance/versiones.

El formulario existente añade consentimiento remoto separado, explica datos compartidos y ausencia de acciones nuevas, y conserva la elección ante fallos. La ficha ofrece desactivación aun con Plugins apagado. El diálogo de actividad añade estado, motivo y actualización local de una valoración pendiente; consultar no ejecuta, reanuda o reconcilia. No cambian sidebar, materiales, iconos o navegación general. El puente FRB conserva su forma; los datos viajan por el JSON de ejecución existente.

El bot puede descubrir resultados guardados sin aviso mediante `assignments_execution_list`, con cursor exclusivo y hasta cincuenta resúmenes sin cuerpos/argumentos. Valida el origen y la instancia actual del encargo; IDs reutilizados no muestran la instancia anterior. `assignments_execution_get` conserva la lectura del resultado compartido por su referencia. Estas consultas no repiten funciones ni activan el calendario; una lista vacía no acredita ejecución o ausencia de novedades. La retirada descrita aquí afecta futuras valoraciones/avisos automáticos, no borra resultados históricos de la consulta explícita autorizada del usuario.

Pruebas de propietarios cubren cuatro decisiones, permisos/consentimiento, conservación de versiones, falla de reserva/ACK, consejo inválido, indisponibilidad, interrupción/reapertura, migración 0008→0009 y datos anteriores, presupuestos/origen y revocación antes/durante IO. Una prueba de composición recorre outbox→adaptador→worker→resultado guardado con guía indisponible, sin modelos o efectos. Proactivity comprueba configuración preparada y revocación antes del transporte; su servidor local de prueba usa datos ficticios.

Los widgets verifican consentimiento/error/retirada, consulta sin efectos y claro/oscuro a 1100×720, 640×420 y texto 150 %. La sonda FRB verifica estado y retirada reales sobre SQLite temporal con datos preparados, sin modelos ni funciones externas. Cifras finales, capturas y compilación en testing y el plan. No equivalen a calidad de decisiones del modelo, VoiceOver físico o material nativo. El corpus comparativo, refinamiento conversacional, auditoría completa y copia histórica siguen pendientes.

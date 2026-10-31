# D-032 — Resultados de Encargos entregados por el bot

29 de octubre de 2026. Continuación de D-029 y D-031. La entrega de cada resultado se elige expresamente en la configuración de ejecución; empieza apagada y no cambia políticas existentes. Es un canal durable para informar hechos, todavía sin selección contextual de la guía.

## Propiedad y consentimiento

Assignments añade 0007 (namespace events) para su outbox; 0001..0006 son inmutables. La política guarda opcionalmente el destino completo validado por bootstrap: conversación, bot, época e hilo de proveedor. Ausencia significa sin avisos. Abrir el formulario consulta el destino sin conectar al proveedor ni inferir; el usuario decide si comparte resultados y contexto con el modelo configurado del bot. Guardar vuelve a comprobar ese destino y rechaza un chat borrado/reabierto. No se concede esta opción mediante herramientas MCP del modelo.

La política captura ese consentimiento por ocurrencia. Los resultados de ejecuciones ya iniciadas pueden llegar después de pausar o cambiar la configuración; no se reenvía el efecto. La interfaz lo explica. Eliminar el encargo conserva su actividad anterior y también lo informa. No se elige otra sesión al terminar ni se cambia la sesión activa del usuario.

El ejecutor es un módulo determinista, no un arquitecto helper nuevo. Bootstrap adapta su Notice al Update tipado de llm::notices. No se usan identidades, trabajos o tablas del constructor para representarlo. LLM 0008 posee el inbox de estos hechos y Presence 0017 la atribución del mensaje; no modifica 0005..0007 de llm ni 0001..0016 de presence. Los módulos conservan su SQL y la conexión compartida; kernel no recibe datos nuevos.

## Transacciones y recuperación

Terminar el intento y crear su aviso confirman en la misma transacción. La clave assignment-execution:run_id es estable; documento y destino son inmutables. Se incluyen referencia de instancia/política/revisión, fecha, estado y razón fija. Solo un éxito informado puede incluir la observación de la función, limitada a ocho mil caracteres; ausencia, truncamiento u omisión se conservan. Un resultado incierto no pasa IDs internos del diario MCP al prompt. No se guardan errores libres.

El mismo Runner entrega pendientes antes de atender próximas ocurrencias. El receptor confirma su inbox antes de retirar el pendiente del productor. Una respuesta perdida o fallo al confirmar el ACK repite solo la recepción idempotente, nunca la función. Con receptor apagado o inbox lleno, conserva el outbox y las ejecuciones autorizadas siguen según su política. El contador de la ficha dice entrega pendiente de confirmar, no afirma que un mensaje ya llegó al usuario.

Si Plugins está apagado pero Assignments y conversación siguen activos, el Runner atiende únicamente el outbox: no avanza ni despacha funciones, expone runner_active=false y mantiene exclusión de instancia. Si también está apagada conversación, queda la vista pasiva. Activar módulos sigue requiriendo reinicio; no se habilitan automáticamente. El calendario conserva sus reglas de retraso al volver a habilitar ejecución.

La recuperación solo considera reservas interrumpidas: running se vuelve uncertain y reserved se cancela, con su aviso en la misma transacción cuando estaba autorizado. No recorre ni comunica retrospectivamente todo el historial. Un fallo al guardar resultado/outbox revierte ambos; recuperar no repite el efecto. Los avisos independientes no se sobrescriben por llegar otro más reciente. La cola en memoria es acotada; el inbox comparte límite de 512 pendientes con el constructor. El outbox durable puede crecer como el diario si el consumidor no está disponible; no se promete entrega inmediata ni espacio ilimitado.

## Mediación y mensaje

El worker serial existente del bot atiende constructor y ejecuciones; no se crea otro worker, proceso permanente, HTTP ni bus. Para redactar usa el perfil real del bot en sesión efímera, las mismas restricciones sin herramientas/archivos/comandos/búsqueda y treinta segundos por turno. Solo recibe contexto acotado de la sesión de origen y su observación de función como datos no confiables. El prompt distingue éxito informado, rechazo, omisión, cancelación e incertidumbre; no afirma verificación externa ni autoriza reintentos.

Antes de inferencia reserva configuración/revisión, fecha y estado. Guarda duración y salida JSON validada (hasta 1200 caracteres), sin calcular coste o tokens inventados. Error, interrupción o fallo posterior al modelo no originan otra inferencia automática: conserva la salida lista o entrega un texto fijo de Gennie basado en hechos del host. Al cerrar se rechazan entradas nuevas, cancela el hijo propio y une el worker antes de SQLite; la redacción interrumpida queda lista para entregar al reabrir.

Presence revalida conversación/bot/época/hilo al confirmar mensaje assistant, referencia y recibo del inbox en una sola transacción prestada. Borrar y reabrir la sesión invalida resultados viejos sin resucitar contenido. La entrega no copia literalmente el texto de un helper ni convierte datos de una herramienta en instrucciones. El bot principal permanece obligatorio/habilitado por contrato; apagar el módulo de conversación es el control real de lifecycle.

## Interfaz

ChatEntry.execution mantiene identidad/revisión/run_id y estado original como metadatos distintos de delegation o de una llamada MCP de ese turno. Debajo de la burbuja se muestra estado del host y «Ver ejecución del encargo», conservando materiales, tipografía y navegación. Una sesión distinta de la activa conserva la leyenda de procedencia. No se atribuye al mensaje una ejecución nueva ni se deduce evidencia a partir de lo que escribió el modelo.

Ficha e historial del chat comparten el mismo diálogo de intento: carga/error/reintento, función y parámetros originales, resultado y revisión humana sin elección inicial. El enlace comprueba ID, instancia, política, revisión, intento y estado antes de mostrar el contenido. Una instancia reutilizada o referencia incompatible no permite revisar otro encargo. Consultar y comprobar nunca ejecutan ni reanudan. Mantiene comprobación separada del diario MCP cuando corresponde, sin afirmar una resolución conjunta.

La configuración añade únicamente el consentimiento de resultados dentro del formulario existente. La barra lateral y el estilo general permanecen iguales. VoiceOver, entrada física y material nativo siguen siendo comprobaciones separadas de widgets/capturas.

## Evidencia y pendientes

Los tests cubren opt in, destino obsoleto, resultado/outbox atómicos, ACK perdido, receptor no disponible, recuperación de archivo, recepción idempotente, origen/época, rollback de mensaje/recibo, salida inválida/interrumpida y cierre. La prueba de composición usa una respuesta preparada ficticia para recorrer outbox→inbox→worker→Presence con Plugins apagado, sin modelos ni efectos; conserva selección y no duplica al reabrir. Los widgets prueban consentimiento, referencia exacta, errores recuperables y conservación de elección.

Evidencia visual, sonda FRB, compilación y cifras finales en testing y el plan. No equivalen a comprensión semántica general, una operación externa real ni interacción física/VoiceOver. La sonda de transporte/contador de D-031 conserva su alcance propio.

La incorporación de estos mensajes al contexto principal continúa en [D-033](provider-context.md), antes de la siguiente entrada explícita y con recibos persistidos.

D-034 incorpora selección contextual consentida por política y retirada local. Pendientes: evidencia/procedencia ampliada de plugins y auditoría de todas las rutas, evaluación empírica y copia histórica. No declarar completado el goal por el canal de entrega local.

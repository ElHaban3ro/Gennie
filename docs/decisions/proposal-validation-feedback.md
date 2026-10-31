# D-055 — errores de propuestas con índice y corrección explícita

29 de octubre de 2026. Continúa el fallo conservado de [D-052](evaluation-contract-gate.md). Cambio local en validación e instrucciones del bot, con el mismo esquema y dominio de aceptación. Sin UI, barra lateral, SQL, migraciones, FRB, dependencias o permisos nuevos.

## Fallo observado y cambio

La prueba breve autorizada guardó cero borradores: al proponer retirar un duplicado, el segundo candidato tenía document=null y questions=[]. El error genérico no identificaba el candidato y el bot terminó explicándolo sin corregirlo. Que esto causara toda la conducta del modelo es una hipótesis; las instrucciones también omitían cómo representar una pausa. El resultado bruto anterior se conserva sin reemplazarlo por respuestas escritas a mano.

Store devuelve ahora invalid_proposal_input con índice y campo para dos rechazos previos a reservar/guardar: documento ausente sin preguntas y más de ocho preguntas. El mensaje afirma que ese envío no guardó el lote ni consultó la guía. No afirma que nunca hayan existido borradores o consultas anteriores. Los demás errores no adquieren esta clasificación; red, guía fallida/interrumpida y guardado incierto no permiten repetir operaciones.

Las instrucciones conversacionales y la herramienta describen el documento completo con status=paused, conservando los demás campos, para proponer pausar un duplicado. related_ids o el motivo no sustituyen el documento. Ante ese error concreto, el bot puede corregir el lote entero dentro de la tarea autorizada si la revisión sigue vigente, en waiting_bot y sin borradores. No debe eliminar candidatos, inventar valores/preguntas, aplicar propuestas, crear otra revisión o ampliar consentimiento. Si falta una decisión real, pide el dato necesario. No se añadió recuperación automática al backend.

## Evidencia local y límite semántico

La regresión falló antes por el error genérico. Usa Store/SQLite reales y el registro MCP: el segundo candidato inválido señala proposals[1].document, deja waiting_bot y cero borradores/consultas; nueve preguntas señalan proposals[0].questions. El consumidor del test repara explícitamente el segundo documento para pausarlo: quedan dos borradores, una consulta de guía simulada, reenvío idéntico idempotente y documentos/revisiones originales intactos. La prueba no contiene modelos ni una consulta OpenRouter real.

Setenta pruebas de Encargos y once Python correctas. Workspace Rust: 406 correctas y 27 optativas omitidas. Clippy workspace/all-targets, formato, invariantes y diff correctos. Biblioteca/helper debug y paquete release compilados por los lanzadores; firma ad hoc verificada. Cierre técnico en proposal-feedback-finish.json bajo .verification/assistant-chat-real. No se repite la suite Flutter sin cambio Dart. Estos resultados no prueban que el modelo ahora corrija sus argumentos, que la guía sea útil o que un formato sea superior.

## Comprobación preparada, sin autorización implícita

prepare --scope submission --with-guide --budget-minutes 2 prepara duplicate_scope ficticio, solo flat_v1 y estrategias local/bot con guía: máximo un turno gpt-6-luna/low y una consulta openai/gpt-6-luna-decisions. Una repetición, sesenta segundos por celda y dos minutos totales más cierre acotado; se detiene ante fallo. Corregir argumentos antes del guardado puede ocurrir dentro del turno, sin iniciar otro turno o repetir guía. Esta muestra no compara formatos ni calidad general y no reemplaza revisión humana o comparación empírica.

Los once tests Python comprueban también esos límites; baseline y protocol verifican datos/contrato sin cuenta o inferencia. Fuentes/binarios se sellan al preparar el plan. La ejecución remota requirió aprobación nueva del plan exacto: el permiso anterior terminó al primer fallo, y sus tres turnos máximos no usados no autorizaban este contrato modificado. La preparación inicial no inició modelos. Goal activo, sin commit/push, instalación o despliegue.

Preparación 57016e42-385b-47aa-8a40-df19b84263c6: doce filas de control local (un resultado, nueve abstenciones y dos controles de contrato), una fila protocol_verified, cero turnos/guía y ningún check fallido o registro malformado. Los dos esquemas coincidieron exactamente al descubrirse, en 449 ms observados. Las 208 fuentes y los binarios quedaron sellados; archivo privado sources.tar.gz sin .env o autenticación. El plan permanece separado de la campaña anterior.

## Ejecución autorizada y cerrada

El usuario autorizó «Autorizar prueba de un turno». Run 57016e42-385b-47aa-8a40-df19b84263c6 completó un turno y una consulta de guía, con dos borradores válidos y una sola llamada a assignments_propose: primer envío válido, sin observar reparación tras error. Ningún borrador fue aplicado; planes originales, recurrencia y rutas de autorización pasaron sus comprobaciones. Fuentes/binarios sin deriva al cerrar. AUTHORIZATION.json, ANALYSIS.json, CLOSURE.json y RESULT.md conservan alcance y evidencia; el permiso terminó al completarse. No hay otra campaña autorizada.

Duración total observada 16.685 ms, proveedor 16.412 ms y guía 1.712 ms. Uso acumulado una vez por hilo: 71.450 tokens de entrada (58.624 caché incluidas), 555 de salida y 72.005 totales; cero reintentos de proveedor observados. La guía reportó coste 0.0002417; coste monetario efectivo total desconocido. Probabilidades entregadas al bot no equivalen a autorización o verdad. El borrador pausado también reescribió la descripción: observación asistida que debe revisar una persona, no una aceptación del contenido.

Esta muestra prueba un recorrido real de envío/guía en un caso y formato; no demuestra utilidad de guía, recuperación del modelo, calidad humana o formato ganador. ratings.csv conserva puntuaciones vacías. proposal-feedback-live-finish.json amplía el cierre técnico previo a inferencia; no se reemplazan resultados anteriores. Continúan las demás exigencias del goal.

## Preferencia humana después de aclarar las variantes

La primera pregunta resumió de forma ambigua «A pausa el duplicado». El usuario lo entendió como una pausa sin preguntar y no entendió B. Se aclaró que ambas eran borradores y que B añadía cambios al nombre/objetivo del encargo activo y al objetivo del duplicado. No se atribuye esa confusión exclusivamente al modelo: el resumen del evaluador omitió «propone». No se convirtió esa primera respuesta en una elección o puntuación.

El usuario confirmó después «Eso, banco, como tu, la b simplificada». Preferencia para este caso: aclarar el encargo que quedaría activo cuando eso ayude y proponer pausar el duplicado conservando sus demás datos. HUMAN_PREFERENCE.json conserva su elección y PREFERRED_VARIANT.json contiene esa edición del asistente, identificada como variante revisada y no como muestra nueva del proveedor. live.jsonl, los resultados anteriores y ratings.csv no se reemplazaron; no se inventaron notas numéricas, no se aplicaron encargos y no se autorizó otra inferencia.

Las instrucciones conversacionales y la descripción de assignments_propose ahora explicitan ese criterio: no cambiar más campos para aparentar una mejora, no uniformar el objetivo de un duplicado que se propone pausar y describir efectos condicionados a la aplicación humana. No se añade una prohibición rígida a Store que impida cambios expresamente solicitados. La preferencia cualitativa no prueba que el modelo ya siga esas instrucciones ni que haya un formato ganador. Los demás casos/comparación y la revisión humana más amplia mantienen su alcance pendiente.

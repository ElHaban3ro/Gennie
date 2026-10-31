# D-030 — Propuestas de Encargos revisadas por el usuario

2026-10-29. Continuación parcial de [chats y agentes](../plans/gennie-chats-agentes.md), sobre D-025 y D-029: selección humana, borradores del bot, valoración opcional de la guía y aplicación transaccional. La ejecución periódica continúa en D-031 y el refinamiento desde respuestas del chat en D-035. No demuestra superioridad semántica de un diseño de datos.

## Propiedad y alcance

Se conserva Document: objetivo, contexto, método, autorización, entrega y preparación ya tienen responsabilidades distintas. No se reescriben encargos ni las migraciones 0001..0004 para simplificar sin mediciones. 0005 añade assignment_reviews y assignment_proposals al namespace histórico events. Assignments posee su SQL; no depende de llm, presence o proactivity. Bootstrap adapta origen y clasificación mediante callbacks. Kernel conserva la única conexión sin nuevos datos de negocio.

La selección fija entre uno y veinte encargos con ID, instance_id y revisión esperada, comprobados en la transacción de creación. Un ID reutilizado no hereda revisiones. Scope conserva documentos, orientación, conversación/época y decisión humana use_guide, con límite de 256 KiB. request_key idempotente no admite otro alcance. FRB expone instance_id como referencia técnica sin mostrarlo en el producto.

Cada lote contiene hasta seis propuestas, un cambio compuesto por encargo: título, motivo, beneficio, referencias relacionadas dentro de la selección, preguntas y documento completo propuesto u omisión si falta un dato imprescindible. Se valida con las reglas del plan; el lote tiene límite de 96 KiB. No modifica el encargo. Los documentos de borrador son inmutables; solo cambia su resolución humana y recibo.

## Usuario → bot → guía → bot

La interfaz humana permite disponibilidad, inicio, listado, aplicación/descarte y cancelación. D-037 permite iniciar selección local por MCP desde una petición real del chat; no permite crear consentimiento remoto, aprobar o cancelar. El diálogo conserva las revisiones observadas al abrir, desmarca la guía por defecto y explica qué datos se compartirían con OpenRouter. Comprueba configuración/habilitación antes de aceptar una valoración remota.

El botón prepara la revisión y envía una petición natural al bot de la conversación activa, sin IDs internos en el texto visible. assignments_review_get sin ID obtiene la selección pendiente del mismo origen; con ID exige también conversación/época. El bot guarda borradores con assignments_propose. Mientras su sesión tenga una revisión pendiente, create/update MCP del bot se rechazan dentro de sus transacciones: no puede aceptar sus propios cambios por otra herramienta. Las ediciones humanas siguen disponibles e invalidan propuestas anteriores.

Solo use_guide autorizado por esa selección permite invocar el callback remoto. Lote y estado evaluating se reservan antes del IO; repetir el mismo envío no repite la clasificación. La guía recibe únicamente documentos seleccionados, orientación y propuestas, con preguntas de utilidad, duplicación/conflicto y cambio de alcance. No recibe conversaciones ni contenido de fuentes consultado por el bot. La respuesta noul exige claves/tipo previstos y probabilidades finitas en [0,1]. No se aplica un umbral universal ni se ejecutan/filtran propuestas por esas probabilidades. El consejo vuelve al bot, que comunica su interpretación al usuario.

Se conserva consejo estructurado validado, duración y metadatos de modelo/uso/coste reportados si existen; no se inventan costes ni se guardan errores libres o credenciales. Fallos producen guide_failed con borradores intactos; reiniciar una valoración produce interrupted sin repetirla. El cliente conserva plazo de treinta segundos y corre en el worker MCP, sin lock SQLite ni bloqueo visual. No hay consultas al arrancar o guardar perfiles. El cierre de módulos/host espera operaciones antes de DB.

Contrato contrastado con [la respuesta noul de OpenRouter](https://openrouter.ai/blog/insights/what-is-jev/) y [Decisions integrado](https://openrouter.ai/openai/gpt-6-luna-decisions/) el 29 de octubre: decisiones tipadas, no texto generativo. Gennie conserva redacción y comunicación. El contrato HTTP local no demuestra disponibilidad de cuenta ni calidad en nuestros encargos.

## Aplicación y recuperación

La ficha presenta propuestas de Gennie. Revisar muestra motivo/beneficio, preguntas y todos los campos cambiados, también los retirados, con valores actuales/propuestos legibles. Preguntas pendientes o selección editada, borrada o reemplazada impiden aplicar. No se refresca silenciosamente la base. Las aplicaciones previas del mismo lote se reconocen por recibo, permitiendo aceptar cambios sobre encargos distintos uno a uno; una edición externa posterior invalida el conjunto.

Aplicar actualiza documento/revisión y registra resolución en una transacción. Un fallo del recibo revierte el plan; repetir no vuelve a aplicar. Descartar conserva el plan; cancelar descarta borradores y libera la revisión. Una clasificación en curso debe terminar antes de resolver. Aceptar no construye plugins, activa ejecuciones ni envía avisos.

Los diálogos mantienen título/acciones fuera del scroll, borradores tras error, bloqueo durante guardado y foco/desplazamiento del error, con base tonal y tipografía existentes. La barra lateral no cambia. [D-036](proposal-history.md) sustituye el límite inicial de veinte revisiones por páginas y archivo humano por familia; conserva borradores y recibos.

## Evidencia y pendientes

Pruebas: scope/revisión/identidad, origen ajeno, borrador sin edición, aplicación idempotente, varios encargos, contexto cambiado, rollback, preguntas, guía única/fallida/malformada, recuperación desde archivo sin otra llamada, bloqueo MCP de autoaprobación y resolución humana. Runtime recorre API/coordinador/SQLite/reapertura con cero modelos/hilos/trabajos. La migración histórica conserva filas, claves y checksums; el recuento esperado pasa a cinco.

Doce widgets cubren petición natural, consentimiento, conflicto/borrador, campos retirados, preguntas, bloqueo doble y errores enfocados/visibles. Dieciocho capturas y mediciones de contraste, mínimo 5,27361:1, en claro/oscuro, ventana amplia/mínima y texto al 150 %. Cuatro flujos FRB/dylib/SQLite reales aplicaron cuatro propuestas una vez, sin inferencia/cuentas/navegador/trabajos; cerraron/retiraron su fixture. No equivalen a interacción física/VoiceOver. Registros en testing.

El refinamiento natural se incorpora en [D-035](proposal-refinements.md), sin heredar consentimiento de guía ni permitir autoaprobación de lotes pendientes; archivo/páginas continúan en D-036 e inicio local desde chat en [D-037](chat-review-requests.md). Pendientes: evaluación comparada con modelos reales y verificación nativa. El calendario/diario local explícito continúa en D-031; D-032 añade avisos por el bot y D-034 la valoración contextual opt in. [Protocolo de evaluación](../plans/encargos-evaluacion.md). El goal sigue activo.

## Seguimiento nativo: aplicación técnica ficticia

El [recorrido 69224db6](../verification/native-assignment-edit.md) verifica en el release normal diferencias de un borrador precargado, Escape sin resolver, aplicación explícita con un único cambio de descripción/revisión y recibo, y consulta posterior sin acciones para reaplicar. Otra base verifica creación/edición básica. Producto sin cambios, modelos/guía/Runner apagados, datos personales intactos. No es valoración humana de calidad o nueva muestra del modelo. El fallo de serialización del primer informe y la recuperación de su estado completo se conservan con alcance explícito; segundo cierre del driver correcto. Goal activo.

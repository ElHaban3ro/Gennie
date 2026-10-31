# D-035 — Continuar propuestas desde una respuesta del chat

29 de octubre de 2026. Continúa [D-030](assignment-proposals.md). Una aclaración del usuario prepara otra versión del borrador; modificar el encargo sigue requiriendo aplicación humana. No habilita solicitudes nuevas desde el chat sin selección previa, archivo o paginación integral de revisiones.

## Intervención y propiedad

Presence expone la última entrada literal del usuario de la conversación activa de Codex: ID estable, conversación/época, fecha, extracto de hasta 4.000 caracteres Unicode y marca de truncamiento. No usa la sesión seleccionada solo para lectura, mensajes del bot/sistema ni argumentos que afirme el modelo. Bootstrap adapta ese lector al origen de la invocación MCP en curso; sin invocación o con hilo ajeno, falla. No hay SQL de Presence dentro de Assignments.

Assignments conserva ese extracto como contexto de auditoría de la revisión en `assignment_review_inputs`. La migración aditiva 0008 del namespace events añade también `assignment_review_refinements`: relación padre/raíz, número de versión y respuesta de origen. No altera planes, revisiones antiguas ni migraciones anteriores. No hay claves foráneas entre apps: borrar el chat no borra estos extractos de revisión; cambiar su época impide reutilizar el origen desde el modelo. No se copia la conversación completa.

Guardar un lote y vincular su entrada se confirma en una transacción antes de cualquier valoración remota. Un fallo no deja propuestas parciales ni provoca una llamada. Repetir un lote idéntico conserva su recibo, sin cambiar la entrada original. Las revisiones históricas sin entrada capturada siguen consultables/aplicables bajo sus reglas originales, pero requieren una revisión humana nueva para continuar por chat; no se les inventa procedencia.

## Nueva versión y consentimiento

`assignments_review_refine` acepta únicamente el ID de la revisión. El host aporta la respuesta real; no acepta texto, ID de mensaje o consentimiento remoto del modelo. Exige el mismo origen, una entrada posterior a la que produjo el lote, propuestas pendientes, base vigente y revisión hoja sin valoración en curso. Si hay varias revisiones activas, la consulta sin ID devuelve una selección acotada y pide aclaración; no devuelve un alcance arbitrario.

La transacción conserva la orientación inicial, las preguntas/propuestas de la versión inmediatamente anterior y los documentos seleccionados. Solo deja editables los encargos que no fueron aplicados o descartados por el usuario. Reconoce aplicaciones previas mediante sus recibos; una edición ajena o reutilización de ID invalida el contexto. No recompone una base nueva silenciosamente.

Cada respuesta tiene como máximo una versión por raíz y cada padre un sucesor. Repetir la misma petición devuelve la versión existente; el mismo ID con texto/fecha/truncamiento contradictorios se rechaza. La nueva versión empieza `waiting_bot`, sin modificar el encargo ni ejecutar funciones. El bot usa `assignments_propose` para guardar el nuevo borrador. La autorización de guía de D-030 es de un lote: **no se hereda** y el refinamiento siempre tiene `use_guide=false`. Una valoración fallida/interrumpida anterior no se repite.

El padre permanece como historial inmutable, pero no admite nueva entrega, cancelación ni resolución de propuestas pendientes. Las resoluciones ya confirmadas conservan su idempotencia. Solo las hojas pendientes bloquean mutaciones MCP ordinarias, manteniendo la prohibición de autoaprobación. Aplicar desde la interfaz conserva la comprobación y transacción de D-030; ni aclarar ni guardar otro borrador equivale a aplicar. Aplicación humana y refinamiento concurrentes no pueden ganar ambos sobre la misma propuesta pendiente.

## Interfaz y evidencia

Las preguntas ofrecen «Responder en el chat» cuando existe origen vigente, bot disponible y entrada verificable. Solo navega al chat existente; no envía texto ni aplica. Las versiones anteriores se identifican como tales y permiten «Ver versión posterior», con aplicación/descarte deshabilitados. La consulta humana local `get` usa el puente JSON existente, sin cambiar el esquema FRB. La vista busca la propuesta del mismo encargo; ausencia/fallo queda visible y no abre otro borrador por error. Una consulta pendiente bloquea inspectores duplicados. Título y acciones permanecen fuera del scroll; no cambia la barra lateral ni el estilo general.

Pruebas de propietario, composición de producción, Flutter y sonda FRB/SQLite descritas en [testing](../rules/testing.md). La composición usa filas de usuario reales de un fixture y las herramientas conectadas por el adaptador de producción; la sonda visual prepara versiones ficticias y prueba lectura/aplicación. Ninguna de ellas demuestra interpretación semántica de un modelo o interacción física/VoiceOver.

Archivo/páginas continúan en [D-036](proposal-history.md) e inicio local desde chat en [D-037](chat-review-requests.md). Pendientes: evaluación comparada de utilidad, auditoría completa de comunicaciones/procedencia de plugins, copia histórica real y matriz nativa. El goal general continúa activo.

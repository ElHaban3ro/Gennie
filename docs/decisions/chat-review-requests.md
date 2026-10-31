# D-037 — Solicitar propuestas en el chat

29 de octubre de 2026. Integrado localmente. Continúa D-030/D-035/D-036 y ajusta la decisión inicial de exigir selección previa en la ficha para cualquier borrador. Una petición explícita del usuario en el chat puede preparar una revisión local: el bot interpreta su alcance y selecciona referencias vigentes de los encargos registrados. No se añade un clasificador por palabras ni un formulario obligatorio al envío de mensajes.

El host toma la intervención real y el origen de Presence durante la invocación MCP; el modelo no aporta texto, IDs de mensajes, época o consentimiento remoto. El inicio usa una clave por intervención y guarda revisión/fuente juntos. Una intervención no puede iniciar dos selecciones distintas; un reintento conserva su recibo y no reactiva archivos. La interpretación de intención/selección corresponde al bot y debe evaluarse semánticamente; la fuente acredita procedencia, no una autorización criptográfica del lenguaje natural.

Esta vía solo prepara borradores locales y siempre usa `use_guide=false`. Valoración remota sigue requiriendo selección y consentimiento explícitos en la interfaz. Aplicar/descartar/archivar siguen siendo humanos. No se editan encargos, activan funciones ni heredan permisos.

## Contrato y persistencia

`assignments_list` añade instance_id a los resúmenes. `assignments_review_start` acepta entre una y veinte referencias id/instance_id/revision, sin documentos nuevos, orientación, request_key, origen, IDs de mensajes o guía. El host obtiene BotTurn mediante Presence durante la invocación MCP y comprueba su origen. Ambigüedad o más de veinte encargos se aclaran con nombres reconocibles, sin IDs técnicos.

La orientación es el extracto literal del usuario, hasta 4.000 caracteres Unicode y marca de truncamiento. Referencias ordenadas por ID y clave `chat:conversación:época:mensaje` permiten un reintento idéntico. La migración aditiva 0010, namespace events, añade `assignment_chat_review_requests`, conserva 0001..0009 y no inventa procedencia histórica. Crear revisión/fuente y comprobar identidad/revisión es una transacción; fallo no incrementa la revisión publicada. Repetir exige alcance, texto, fecha y recorte originales y no reactiva archivos. Una clave humana histórica coincidente no se reclasifica como petición del chat. La intervención que produce el lote no puede preceder a su solicitud.

D-035 conserva las versiones y sus respuestas; todas consultan la solicitud original por raíz. La fuente es contexto de auditoría de Assignments, sin FK a Presence ni conversación completa: persiste al borrar el historial, y cambiar época impide reutilizar el origen desde el modelo. No se trata texto como instrucción de sistema ni consentimiento remoto. La fuente acredita procedencia, no la corrección semántica de la selección.

## Interfaz

La revisión muestra «Solicitada desde el chat», «Solicitud del chat» y, al continuar, «Respuesta para esta versión». Textos largos usan ExpansionTile, resumen de dos líneas y extracto desplegable para no desplazar los cambios. El umbral de 400 caracteres es de presentación, sin intervenir en decisiones/permisos. Se identifica recorte, sin afirmar que el extracto sea el original completo. Título/acciones fijos y cuerpo desplazable; barra lateral, marcos y estilo general conservados.

AssignmentProposals observa executionRevision además del historial; el coordinador publica el contador compartido después del commit. Las revisiones/borradores se actualizan antes de otra respuesta, sin mensajes artificiales. No cambia el esquema FRB. El bot explica una lista de cambios y su revisión humana en Encargos, sin IDs internos.

## Evidencia y límites

Seis pruebas nuevas de propietario cubren fuente literal, canonización/reintento, selección/texto/fecha contradictorios, guía imposible, lote anterior a solicitud, versiones/archivo, revisión/ID reutilizado, rollback, concurrencia, clave histórica y migración 0009→0010/reapertura/checksums. La composición de producción recorre catálogo→inicio MCP con fila real de usuario→pregunta→respuesta→versión→aplicación/archivo humanos. Rechaza origen/guía falsificados, conserva fuente al reabrir y publica metadatos sin otro mensaje.

Sonda optativa `real_model_starts_local_review_only_after_explicit_request`: dos respuestas ChatGPT con instrucciones del bot en una sesión técnica aislada, handlers reales y origen de fixture. Una observación con negativa expresa no creó revisión; una petición posterior por dos encargos creó una revisión y dos propuestas revisables, sin intentar editar planes. Turnos: 3.080/20.065 ms; prueba completa 25,19 s, watchdog 60 s. Datos ficticios, archivos/comandos/acciones locales deshabilitados y escritura de planes rechazada por el harness. No sustituye la conversación principal ni prueba toda su cadena proveedor/diario, ambigüedad general, utilidad o coste monetario. No se repitió como campaña.

Ocho widgets nuevos/45 focalizados, doce capturas de widgets y cuatro Flutter/FRB/SQLite inspeccionadas. Dieciocho mediciones de controles, mínimo 5,25780:1; claro/oscuro, 1100 × 720, 640 × 420 y texto 150 % en widgets. La sonda del puente prepara solicitudes/borradores ficticios para comprobar lectura literal y aplicación humana; no genera propuestas con un modelo. Registros en testing. No acredita material físico, entrada nativa o VoiceOver.

Pendientes generales: evaluación comparada/coste y variantes semánticas, auditoría integral/procedencia de plugins, copia histórica real/rendimiento, comprensión real de citas y matriz nativa. El goal continúa activo, sin commit/push o despliegue.

# D-052 — contrato descubierto y prueba breve de Encargos

29 de octubre de 2026. Continúa [D-050](proposal-portability.md). Cambia únicamente herramientas de evaluación y pruebas; no el runtime del producto, UI, sidebar, SQL, permisos o dependencias. No consume nuevos turnos/guía por preparar o verificar el protocolo.

## Problema y comprobación

El protocolo anterior solo comprobaba nombres disponibles. Un esquema correcto en la definición Rust no demostraba que Codex lo hubiera descubierto íntegro. El fixture ahora entrega al worker los inputSchema exactos de las herramientas registradas. El worker los compara con los de mcpServerStatus/list antes de generar y conserva hashes SHA-256 de JSON canónico por herramienta. Una diferencia, incluso entre false y cero, detiene el turno como mcp_schema_mismatch. No guarda datos de autenticación, descripciones o argumentos en esa evidencia.

El protocolo real de 187c5232-783e-4540-82d2-27012918e111 pasó: assignments_propose y assignments_review_get llegaron idénticos al inventario de Codex, conexión observada 203 ms, cero turnos/mensajes/consultas de guía. Se usaron app-server, stdio y socket privado propios, con perfil sin cuenta, sin HTTP entrante ni UI. Esto acredita descubrimiento; no observa el formato interno que el servicio entrega al modelo ni demuestra que vaya a respetarlo. No permite atribuir definitivamente los fallos de la campaña anterior a un renderer.

## Alcance breve, separado de una comparación

prepare --scope smoke --with-guide --budget-minutes 5 prepara un solo caso de duplicado ficticio, dos formatos y estrategias local/bot/bot con guía: máximo cuatro turnos gpt-6-luna/low y dos consultas openai/gpt-6-luna-decisions. Una repetición; no amplía el corpus ni sustituye la comparación pendiente de cuatro casos. La ejecución remota requiere permiso nuevo: los dieciséis turnos anteriores ya se consumieron.

Este modo detiene las celdas restantes al primer fallo de modelo, ausencia de borrador, comprobaciones desconocidas/fallidas o guía no confirmada; quedan not_run_after_model_failure con cero turnos/consultas, sin repetir o reemplazar el fallo. Exige una revisión ready con borradores y comprobaciones automáticas verdaderas; bot no consulta guía, bot_guide exige una consulta y resultado confirmado. No interpreta esto como calidad semántica, aceptación humana o formato ganador. Los modos piloto/completo conservan sus muestras independientes, sin esta detención por defecto.

## Evidencia y autorización

Diez pruebas Python correctas cubren integridad del inventario, ausencia/cambio de esquema, exclusión de autenticación y bloqueo de turn/start ante diferencia, más el presupuesto de la preparación. Una regresión Rust cubre los criterios de continuación y detención ante borrador ausente, estado desconocido, fallo y guía no confirmada. Las nuevas pruebas fallaron antes del cambio correspondiente. Suite Rust: 404 correctas, 27 optativas omitidas; Clippy workspace/all-targets, formato, invariantes y diff correctos. No se repiten Flutter o compilaciones de producto porque no cambió código runtime.

El directorio .verification/assignment-evaluation/187c5232-783e-4540-82d2-27012918e111 contiene plan revisable, 208 fuentes selladas sin secretos, hashes de binarios, 22 controles locales y una fila protocol_verified sin deriva. summary.json no tiene comprobaciones fallidas o filas malformadas. APPROVAL_REQUEST.json declara live_authorized=false. Logs evaluation-contract-* y evaluation-contract-finish.json en .verification/assistant-chat-real.

El coste monetario efectivo es desconocido; se conservará uso/coste reportados sin tratar ausencias como cero. No mezclar la futura prueba con la campaña fallida como si fueran equivalentes. Al cambiar fuentes/binarios relevantes, preparar otro plan antes de ejecutar. Goal activo: permanecen evaluación semántica, revisión humana y resto de requisitos del plan; sin commit/push, instalación o despliegue.


## Prueba breve autorizada y cerrada

El usuario autorizó este plan y la prueba terminó al primer fallo: un turno de modelo y cero guía en 15,198 s. Un control local completó; el modelo presentó dos candidatos con los metadatos completos, pero el segundo carecía de documento y pregunta, por lo que todo el lote fue rechazado. Cuatro filas quedaron not_run_after_model_failure, incluidas tres celdas de modelo no ejecutadas. No se reutiliza ese máximo restante para otra campaña. AUTHORIZATION.json, ANALYSIS.json, CLOSURE.json y RESULT.md conservan resultado y fuente anterior a D-054, sin deriva al cerrar. Uso acumulado una vez por hilo: 68.061 entrada (41.472 caché incluidas), 501 salida, 68.562 totales; coste monetario desconocido y cero reintentos observados. Descubrimiento exacto no implica cumplimiento del esquema. Sin borrador válido ni consulta de guía, comparación/calidad humana pendientes.

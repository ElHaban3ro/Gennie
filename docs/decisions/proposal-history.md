# D-036 — Archivo humano y páginas de revisiones

29 de octubre de 2026. Integrado localmente; continúa D-030/D-035. Assignments conserva el archivo de una familia de revisiones por raíz en la migración aditiva 0009, namespace events. Las migraciones anteriores permanecen inmutables. Archivar exige acción humana, conserva documentos/borradores/recibos y no modifica ni ejecuta encargos. No puede archivar una valoración en curso. La familia archivada queda consultable, sin continuar ni resolver propuestas pendientes; una revisión nueva requiere selección/consentimiento nuevos. No se ofrece restaurar la autorización de una revisión archivada.

## Propiedad y transacciones

`assignment_review_archives` tiene una fila por raíz y fecha UTC. Obtener la raíz, comprobar todas sus versiones, registrar el archivo y leer el resultado comparten transacción. Repetir conserva la fecha original; fallo revierte el archivo. No cambia resolución de propuestas, documentos, consentimientos originales o recibos de aplicación. Las resoluciones ya confirmadas siguen reconociendo sus recibos idempotentes.

Las consultas pendientes y el guard contra autoaprobación excluyen familias archivadas. Guardar/refinar/cancelar una revisión archivada y resolver una propuesta pendiente se rechazan; el modelo solo puede consultar sus revisiones del mismo origen. Archivar no es herramienta MCP: únicamente está en la API humana local. Puede operar con conversación o Plugins apagados, sin preparar modelos. Un nuevo intento usa selección, base vigente y consentimiento nuevos.

## Páginas y UI

La ficha consulta revisiones actuales o archivadas por cursor estable `(created_at,id)`, limitado a la instancia del encargo y el modo solicitado. Diez revisiones y presupuesto de 512 KiB para sus valores JSON por página; la primera se entrega íntegra para garantizar avance. Las páginas incluyen total actual y cursor exclusivo, sin depender de que la fila del cursor siga en la vista. Añadir revisiones nuevas no las intercala en páginas anteriores. Cambiar de instancia o modo invalida el cursor. No se presenta el total como una instantánea congelada; la consulta puede reflejar cambios concurrentes. El filtro usa solo SQL de Assignments sobre sus alcances inmutables, sin deserializar todos los documentos en Rust.

La ficha agrupa los borradores por revisión y ofrece «Revisiones archivadas» / «Revisiones actuales» y «Cargar revisiones anteriores». «Archivar revisión» confirma el alcance y los encargos incluidos, también cuando se inicia desde una versión posterior. No envía mensajes. Las revisiones archivadas muestran consulta y bloquean aplicar/descartar; no se describen como una edición externa. Para continuar se prepara una revisión nueva.

Un fallo al cargar conserva filas/cursor para reintentar. Respuestas tardías de otra selección se descartan por generación. Mientras se archiva, las actualizaciones del chat no levantan el bloqueo de escritura ni habilitan un segundo envío. Tras cargar páginas anteriores, un aviso de actualización permite refrescar explícitamente conservando la lectura; actualizar o cambiar de vista reinicia las páginas. Título/acciones de diálogos permanecen fuera del scroll; se conserva la barra lateral, marcos, tipografía y estilo general. Se amplía el JSON de `proposal_request`, sin cambiar el esquema FRB ni regenerarlo.

## Verificación y límites

Pruebas de propietario cubren fechas iguales, inserción entre páginas, fila del cursor archivada, presupuesto con selecciones válidas grandes, identidad reutilizada, rollback, exclusión de pendientes, conservación de recibos y rechazo durante IO de guía. Migrar 0008→0009 y reabrir una copia propia conserva documentos y checksums anteriores. La composición de producción usa origen/entradas reales del fixture, versión, aplicación humana y archivo de toda la familia; no hay modelos o efectos externos.

La sonda `scripts/probes/proposal_history_bridge.py` recorre cuatro casos Flutter→FRB→SQLite con 24 revisiones por encargo, tres páginas y archivo humano. Conserva 96 borradores pendientes, cuatro documentos/revisiones y cero inferencias/ejecuciones. Doce capturas del puente y doce de widgets inspeccionadas; dieciocho mediciones de controles, mínimo 5,25780:1, en claro/oscuro y ventana amplia/mínima, incluyendo texto al 150 % en widgets. Registro y cierre en [testing](../rules/testing.md). No prueba material nativo, entrada física, VoiceOver, rendimiento sobre la base histórica real ni utilidad semántica.

El inicio local desde chat continúa en [D-037](chat-review-requests.md). Comparación empírica, auditoría integral/procedencia de plugins, copia histórica real y matriz nativa siguen pendientes. El goal general permanece activo, sin commit/push o despliegue.

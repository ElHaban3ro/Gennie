# D-046 — contrato publicable de borradores tras el piloto

29 de octubre de 2026. Implementado/verificado localmente; goal activo. Continúa [D-038](assignment-evaluation.md). No modifica la estructura persistida, permisos o resultados del piloto.

## Hallazgo y corrección

El piloto autorizado produjo tres lotes rechazados: un elemento sin documento/pregunta y dos seguimientos sin cadencia interna. Store ya rechazaba esos datos antes de escribir, pero el esquema MCP publicado los aceptaba. La regresión publicada sobre los argumentos ficticios reales del piloto falló antes del arreglo con «El esquema publicado admite el lote rechazado: duplicate_scope».

proposals/schema.rs concentra el esquema del lote. Exige un documento o al menos una pregunta por elemento; conserva la compatibilidad de un documento acompañado por preguntas. Publica los límites existentes de texto/preguntas/referencias, sin eliminar elementos ni completar valores. tools::document_schema añade condiciones para follow_up→preparation.interval_minutes/rationale y para intervalo entero→justificación. Null/ausencia sin seguimiento siguen permitidos. Es el mismo esquema compartido por crear, actualizar y proponer; Store conserva su validación independiente de referencias, contenido, contexto, permisos y transacciones.

La descripción de assignments_propose aclara que un elemento incompleto rechaza todo el lote; no añade seguimiento por mejorar un resumen. La hora de entrega no sustituye la cadencia interna. Conservar la existente o proponer una justificada dentro del alcance acordado; si falta información indispensable, formular una pregunta con document=null. No inferir permisos o inventar datos para satisfacer el validador.

Esto hace explícito el contrato para sus consumidores, no incorpora un validador genérico al host MCP ni demuestra que el modelo siempre lo respete. La validación del backend previa a persistencia permanece obligatoria. No cambia la activación de ejecuciones, API FRB, Flutter, UI o barra lateral.

## Evidencia

apps/assignments/tests/fixtures/proposal-rejections-v1.json conserva solo los tres argumentos/documentos ficticios del run 6ccf7929-83ae-4e35-b9af-aac2806ff7e7, con origen declarado. El test valida el esquema real con jsonschema offline y el Store real con SQLite: rechazo coherente, lote entero sin propuestas, reparación explícita y guardado de todos los borradores, conservando documento y revisión originales. La reparación del duplicado aporta documento completo para pausar la copia; las otras dos retiran el seguimiento que no era imprescindible, sin inventar cadencia. Son reparaciones de test, no nuevas respuestas del modelo ni aplicación humana.

Otra prueba cruza condiciones de seguimiento/cadencia/justificación entre esquema y Store: ausencias, null, cero, justificación vacía, intervalo completo y preguntas pendientes. Jsonschema se añade solo como dev-dependency reutilizando =0.60.2/default-features=false del workspace, licencia MIT/MSRV 1.85 ya revisados. Comparación con Cargo.lock archivado del piloto: ningún paquete, versión o checksum cambia; solo esa arista de tests. Sin nueva dependencia runtime de Assignments.

390 pruebas Rust correctas y 27 optativas omitidas; Clippy workspace/all-targets con -D warnings correcto. El protocolo de evaluación con Codex/stdio reales descubre los esquemas actuales sin cuenta/modelos: una celda protocol_verified, cero turnos/guía/propuestas, sin deriva de fuentes. Run técnico ade8b44e-6cec-44d1-bc62-d117a6a8b677 marcado PROTOCOL_ONLY.json: no autoriza otra campaña live. Las fuentes/binarios de un nuevo live requerirían prepararse y autorizarse aparte; la autorización anterior de ocho turnos ya se consumió.

Logs .verification/assistant-chat-real/proposal-contract-{before,owner,workspace,clippy,protocol}.log y proposal-contract-dependency.json. La regresión roja queda conservada. Los lanzadores ./scripts/dev.sh --build-only y ./scripts/build.sh completaron biblioteca/helper debug y paquete macOS release; firma ad hoc profunda/estricta verificada. Logs proposal-contract-{debug,release}.log. No se abrió, instaló o desplegó el paquete nuevo y la instancia Debug previa del usuario se conservó. El manifiesto técnico de protocolo queda histórico después de recompilar el helper, sin autorización de live. Formato, invariantes y diff correctos. Sin cambios SQL, UI, datos del usuario, commit/push o despliegue. No se repite la evaluación semántica ni se presenta este arreglo como mejora de calidad demostrada.

## Continuación

El piloto y sus revisiones siguen intactos. La comparación ampliada fue autorizada y ejecutada después: dieciséis turnos sin borrador válido y cero guía. [D-050](proposal-portability.md) conserva el resultado inconcluso y la corrección posterior de forma del esquema; valoración humana y efecto semántico siguen pendientes. Detener explícito del chat, identidad de operación, eventos tardíos y fragmento interrumpido se implementaron localmente en [D-047](chat-cancellation.md); continúan los restantes requisitos del goal. Ver [plan de cancelación](../plans/chat-cancellation.md).

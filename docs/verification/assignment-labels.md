# Etiqueta de cadencia y contrato del fixture de propuestas

30 de octubre de 2026. Seguimiento del editor básico al mínimo/150 %. No modifica barra, tamaños de campo, fuentes o material. El único cambio runtime es texto/ayuda de AssignmentEditor; Rust añade solo un test, sin cambiar lógica o migraciones.

## Etiqueta completa

La muestra nativa anterior recortó «Revisar cada cuántos minutos (opcional)». La nueva regresión monta GennieApp completo con Inter, 640 × 420 y texto 150 %, claro/oscuro: RenderParagraph.didExceedMaxLines era true en ambos temas. [Rojo](../../.verification/assignment-labels/20261030/red.log).

El campo conserva labelText y estilo, con «Intervalo de revisión» como etiqueta breve. helperText explica «Minutos entre revisiones. Opcional.»; cuando seguimiento hace obligatorio el campo, la ayuda omite Opcional. La regresión exige etiqueta y ayuda completas, nombre/value/isTextField asociados antes y después de editar y valor conservado al alternar seguimiento, sin guardar. Pasaron **25 pruebas Flutter afectadas**, análisis/formato e invariantes. La suite completa de 402/49 pertenece a la puerta previa de pie/foco; no se repite por este cambio de texto. [Resultado](../../.verification/assignment-labels/20261030/RESULT.json).

La copia nativa oscura 640 × 420/150 % mostró completos Intervalo de revisión y la ayuda, que pasó a dos líneas sin reducir fuente. Preparación se abrió sin activar seguimiento y Cancelar no guardó. Es una condición QA de Flutter, sin cambios a preferencias del sistema, no una prueba de VoiceOver. La misma muestra todavía elide parte de la instrucción entre paréntesis de Pasos de preparación: esa y otras etiquetas extensas mantienen una revisión pendiente.

## Diferencia adicional del fixture, no de la ruta pública

El ensayo anterior insertaba el candidato por SQL omitiendo interval_minutes/rationale y otros opcionales. El documento actual, leído como Document, los serializaba con null; el inspector por ello mostraba además Preparación. No se cambia el comparador de producción para ocultar ese caso.

Candidate.document es Option<Document>; submit_proposals valida y serializa el documento tipado antes de persistir. El test nuevo `omitted_optional_fields_are_canonical_in_public_proposal_readback` recorre create→begin_review→submit_proposals→review públicos con opcionales omitidos, sin guía/aplicación, y exige que solo description cambie; preparación/opcionales coinciden y el original/revisión quedan intactos. **Un test Rust y Clippy de assignments --all-targets con -D warnings pasan.** [Registro](../../.verification/assignment-labels/20261030/canonical-readback.log).

La nueva preparación QA conserva esos nulls en su SQL, contrastados con el test de la ruta pública. En el runner real la misma propuesta mostró únicamente Objetivo, «Revisar novedades» → «Agrupar solo los cambios nuevos»; no hubo sección Preparación adicional. Se cerró sin aplicar/descartar. Esto resuelve el diagnóstico de ese fixture; no afirma calidad general o ausencia universal de falsos positivos.

## Cierre y versión

Producto/driver Salir 0, once tablas seleccionadas idénticas, dos mensajes, SQLite íntegra/FK vacías, cero trabajos/ejecuciones/modelos/guía y sin errores conocidos de canal o residuales. Fixture/preferencias UUID y proyecto/copia QA retirados. [Manifest](../../.verification/assignment-labels/20261030/run/manifest.json), [fuentes](../../.verification/assignment-labels/20261030/SOURCES.json), [release](../../.verification/assignment-labels/20261030/RELEASE.json). Las capturas/AX están en el registro Computer Use.

Build/firma local correctos, ejecutable habitual `f13d615c1b79870eb17cd334f6a229358cd48dd07fec794ec213716abfed4684`; sin instalación/despliegue. Backend/helper conservan sus hashes. El estilo general permanece; otras etiquetas, condiciones de formularios y VoiceOver siguen pendientes en la matriz. Goal activo, sin commit/push.


## Seguimiento de las demás etiquetas

La [auditoría conjunta de 21 etiquetas](assignment-label-audit.md) encontró únicamente Pasos de preparación elidido en el recorrido desplegado. Su instrucción pasó a la ayuda, con rojo/verde en ambos temas, 406 Flutter/49 optativas y comprobación nativa 0. No se cambian etiquetas que ya cabían ni se generaliza a controles no construidos en ese recorrido.

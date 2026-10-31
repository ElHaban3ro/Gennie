# D-050 — alternativas completas del candidato de propuesta

29 de octubre de 2026. Continúa [D-046](proposal-contract.md) tras una segunda campaña autorizada. Goal activo. Sin cambio de datos persistidos, autorización, UI o barra lateral.

## Resultado de la campaña sellada

El usuario autorizó cd4264a3-9b10-4b9c-9545-4639313463a9: hasta dieciséis turnos ChatGPT gpt-6-luna/low y ocho consultas OpenRouter openai/gpt-6-luna-decisions, cuatro casos ficticios en dos formatos y veinte minutos más cierre. Las fuentes y binarios quedaron sellados y no cambiaron durante el ensayo. Se completaron dieciséis turnos y cero consultas de guía; todas las celdas del modelo terminaron sin borrador válido. Las ocho celdas de controles locales produjeron dos resultados y seis abstenciones. Los fallos se conservaron sin repetir muestras.

Se observaron 28 intentos de assignments_propose y 31 candidatos: título, motivo y beneficio faltaron en los 31; assignment_id en 27 y related_ids en 29. Los argumentos se reducían principalmente a document o questions. El backend rechazó los candidatos antes de consultar la guía. No hay comparación bot/guía, formato ganador o calidad humana demostrados. Que los controles automáticos no detectaran modificación de los planes de entrada no convierte las celdas sin borrador en éxito.

Uso reportado sumando total una vez por hilo efímero: 1294028 tokens de entrada (1082368 caché incluidos), 9288 de salida, 1303316 totales. Mediana por celda 14,363 s; duración aproximada por sellos de artefactos 229,081 s; cero reintentos observados, sin garantía sobre reintentos internos del servicio. Coste monetario efectivo desconocido. CLOSURE.json, ANALYSIS.json, PILOT.md, live.jsonl y sources.tar.gz conservan resultado/fuentes previos al arreglo en .verification/assignment-evaluation/cd4264a3-9b10-4b9c-9545-4639313463a9. El permiso de dieciséis turnos quedó consumido; las ocho consultas no usadas no autorizan turnos adicionales ni otra campaña.

## Cambio y alcance

El esquema anterior combinaba campos/requeridos del objeto padre con condiciones anyOf parciales. Es válido como JSON Schema, pero obliga a combinar ambas capas. No se observó la representación interna entregada al modelo: la causa semántica exacta sigue sin probarse. El fallo motivó reducir esta dependencia de interpretación sin debilitar el contrato.

proposals/schema.rs publica dos alternativas completas y cerradas: candidato con documento entero y cero o más preguntas; candidato con document=null y al menos una pregunta. Cada una declara los siete campos, todos requeridos. Conserva límites y compatibilidad documento+preguntas. No completa metadatos, descarta candidatos o transforma una respuesta rechazada. Store conserva validación independiente y transacción del lote; no hay cambios SQL/FRB o dependencias.

La [guía oficial de Structured Outputs de OpenAI](https://developers.openai.com/api/docs/guides/structured-outputs) muestra alternativas anyOf con objetos completos, campos requeridos y additionalProperties=false. Orienta esta forma explícita; no prueba que Codex/MCP use ese modo ni su representación interna. document_schema conserva condiciones de JSON Schema fuera del subconjunto estricto: no se declara compatibilidad global con strict=true ni se elimina validación por esa referencia.

## Verificación

Una regresión sobre la definición real de assignments_propose exige que ambas alternativas expongan por sí mismas campos, requeridos y cierre; falló con el esquema anterior. Otra cruza documento/preguntas, omisión de cada campo, propiedades adicionales y formas incompletas observadas. Los lotes rechazados del primer piloto y sus reparaciones explícitas siguen comprobándose contra esquema y Store/SQLite. Esto acredita estructura y rechazo; no demuestra que un modelo cumpla ahora el contrato. La validación semántica posterior necesita una muestra nueva autorizada y fuentes/binarios renovados.

Cierre local: 403 pruebas Rust correctas y 27 optativas omitidas; Clippy workspace/all-targets con -D warnings, formato, invariantes y diff correctos. Los lanzadores dev.sh --build-only y build.sh completaron debug/release; firma ad hoc profunda/estricta verificada. Las 55 migraciones del sello de la campaña permanecen idénticas. No hubo cambios Dart/Swift del producto en esta etapa: la suite Flutter de D-049 (350 correctas, 49 omitidas) conserva su alcance y no se repitió. Los tres recorridos nativos del perfil se hicieron antes de esta recompilación y se identifican por hash en su evidencia.

El run técnico e41affcd-1b50-4062-907e-cfc69887a536 quedó marcado PROTOCOL_ONLY.json y verificó descubrimiento Codex/MCP con perfil privado sin cuenta: una celda protocol_verified, cero turnos/guía y sin deriva. No valida la representación interna del esquema entregada al modelo. No es autorización live; preparar otro plan con fuentes/binarios vigentes antes de una muestra aprobada. proposal-portability-finish.json y logs proposal-portability-* de .verification/assistant-chat-real conservan el cierre. No se aplica ningún borrador a encargos personales, publica código o despliega.

# Avisos contextuales: guía y bot reales

30 de octubre de 2026. [Alcance previo](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/PLAN.md), [resultados](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/live-summary.json) y [cierre](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/CLOSURE.json). Autorización continuada del usuario; sin cambios de producto, UI, barra lateral o datos personales.

## Recorrido y resultados

Primero pasaron los cuatro casos sin clave, cuenta o modelos: guía no disponible, resultado guardado y cero redacciones. Después se comprobó el mismo binario/corpus sellado con **cuatro respuestas ChatGPT gpt-6-luna, esfuerzo low, y cuatro consultas OpenRouter openai/gpt-6-luna-decisions**. Un solo intento por fase y caso, sin reintentos. La fase real duró **25.435 ms**; mediana por caso **6.185,5 ms**.

La sonda atraviesa el outbox de Assignments, el adaptador real NoticeGuide con sus comprobaciones de consentimiento, Proactivity, el worker Codex sin herramientas, el esquema/parser y el diario de Presence sobre SQLite temporal. Las observaciones exitosas están precargadas y son ficticias: no se ejecuta una función para producirlas. Políticas pausadas, Plugins/Browser/dictado/voz apagados.

| Caso fijado antes de ejecutar | Acción esperada | Observada | Entrega |
|---|---|---|---|
| Estado sin cambios; avisar solo si cambia | record | record | Ninguna |
| Pendiente → aprobado | notify | notify | Un mensaje atribuido a la función |
| Dos borradores sin criterio para elegir | ask | ask | Una pregunta A/B |
| Datos completos; ofrecer redactar y esperar | offer_help | offer_help | Una oferta, sin redactar o enviar |

Las cuatro acciones coinciden con el acuerdo explícito. Frente a la expectativa de interrupción fijada para estos casos: **0 falsos positivos entre un caso negativo y 0 falsos negativos entre tres positivos**. La regla local de referencia («notify si cambia; record si no») coincide en 2/4 acciones exactas y en la misma decisión binaria de interrumpir. No se midió su tiempo de ejecución; esta comparación pequeña no demuestra superioridad estadística ni atribuye causalmente el resultado a la guía.

La revisión asistida del texto confirma que el aviso atribuye el cambio a la función, la pregunta no inventa una preferencia y la oferta espera aceptación. [Revisión identificada como IA](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/AI_REVIEW.json); no son valoraciones nuevas del usuario. La guía dio probabilidades altas simultáneas en varias categorías; el bot conservó la elección contextual, sin una selección universal por umbral.

## Persistencia, consumo y límites

Tres mensajes llegaron al origen, sin mover la selección de otra sesión ni incluir su centinela en el texto. Cada inbox reservó una guía y una redacción; el outbox recibió un ACK. Reabrir cada base conservó decisión, contadores y mensajes, sin otra consulta o entrega. Definiciones y políticas permanecieron iguales, cero trabajos/intentos de herramientas o hilos conversacionales del proveedor. Integridad/FK correctas, cuatro bases y sockets retirados, sin rutas temporales o procesos propios residuales.

La guía reportó coste parcial **0,0005763**; el coste completo efectivo sigue desconocido, sin moneda inferida ni coste cero inventado para el bot. [Métricas](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/METRICS.json). Los máximos no usados están cerrados.

Esta evidencia acredita el contrato corregido con los proveedores reales y cuatro acuerdos ficticios. No prueba una ejecución sobre servicios externos, utilidad cotidiana, interpretación de cualquier objetivo, todos los duplicados semánticos o aceptación humana. Las campañas anteriores siguen midiendo borradores, con sus fuentes/versiones y fallos propios. No se repite inferencia para ampliar esta muestra.

La sonda es un test optativo bajo `runtime_tests/contextual_notice_probe.rs`, con plan/acuse explícitos, reclamación única, límites fijos y protocolo previo obligatorio para live. No se añade a las suites automáticas ni al binario de producto; el release a3502e73 conserva sus fuentes de producción y su firma anteriores.

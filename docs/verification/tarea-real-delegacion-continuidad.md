# Tareas reales de auditoría con subagentes

9 de noviembre de 2026. Se realizaron dos turnos reales mediante `Session` de Gennie, con el perfil guardado del bot y `gpt-6.1-terra`/medium. Se copiaron los mismos 31 archivos reales de `llm`, MCP, plugins y documentación a sendos directorios temporales de solo lectura; sus huellas coinciden entre muestras y no cambiaron durante cada ejecución. No se usaron cuentas o servicios personales desde las tareas, ni la interfaz de escritorio completa. El servidor del modelo sí usó la sesión real de Codex.

Se activaron explícitamente los subagentes nativos en el app-server de la sonda, con hasta cinco hilos y profundidad dos. Es una **condición experimental**, no una prueba de que esa opción esté activa en el producto habitual ni de la delegación propia de Gennie al constructor de plugins. La sonda limita la ejecución a 600 s y el proceso exterior a 660 s; el adaptador conserva su política real de inactividad. No se añadieron esperas artificiales. La captura optativa registra un conteo de hijos distintos del hilo raíz y eventos de colaboración, sin conservar sus identificadores; la regresión `project_observation_counts_distinct_children_without_serializing_ids` comprueba deduplicación y privacidad.

## Resultados

| Tarea | Tiempo | Delegación observada | Entrega |
|---|---:|---|---|
| [Auditoría inicial](../../.verification/project-delegation/62de6a09-6f41-427a-8eb4-7f7e76137820/result.json) | 96.174 ms | Tres hijos distintos; tres esperas completadas | Turno completado, siete secciones solicitadas, 37 referencias a líneas existentes del corpus. |
| [Auditoría con revisión cruzada](../../.verification/project-delegation/25fd324b-4688-42c1-bbde-36e95a15fd8f/result.json) | 153.062 ms | Tres hijos distintos; once esperas completadas | Turno completado, ocho secciones solicitadas, 47 referencias a líneas existentes del corpus. |

Ambos turnos terminaron sin error ni vencimiento del Scope, sin guía humana durante la ejecución. El informe de la segunda muestra afirma que hubo revisión cruzada y recoge matices; el observador registra actividad de subagentes y esperas, pero no conserva los mensajes intercambiados, por lo que **no verifica independientemente el contenido de esa revisión cruzada**. La comprobación automática de referencias solo verificó pertenencia al corpus y que la línea existiera: no validó la corrección de cada conclusión. Los enlaces `/tmp` de las respuestas ya no existen tras retirar las copias; las rutas relativas y huellas figuran en cada resultado.

El archivo de sesión asociado al hilo principal registra 361.712 tokens en la primera muestra y 487.779 en la segunda, con caché incluida. Las otras tres sesiones de cada muestra conservan nombres de archivo distintos pero metadatos internos con el identificador del hilo principal; no se suman ni atribuyen como consumo fiable de los hijos. No se calculó coste facturado.

## Hallazgos y límites

En ambas auditorías se identificó que la actividad elegible de un turno renueva el plazo de **todas** sus herramientas activas. La inspección directa de `apps/llm/src/codex.rs` confirma ese bucle: una herramienta silenciosa podría conservar el plazo mientras otra actividad reconocida continúa. Es un riesgo de la política de inactividad actual, no un bloqueo observado en estas muestras. El informe también señala que el constructor cuenta fallos idénticos consecutivos y turnos sin herramientas, lo cual no equivale a comprobar el progreso semántico; las secuencias alternantes requieren reproducción específica. Propuso otras hipótesis sobre efectos equivalentes y recuperación que no se verificaron aquí.

La segunda tarea fue más extensa, con tres fases solicitadas y ocho entregables, pero terminó en 153 s: **no superó los 180 s** y no prueba por sí misma la continuidad del trabajo multiagente más allá de ese umbral. La [sesión real con herramienta controlada](turno-conversacional-inactividad.md) sí superó 180 s, pero no fue esta auditoría multiagente. No se observó interrupción ni abandono en estas dos muestras; tampoco se puede concluir que sea imposible en tareas más largas, con la UI completa, con la delegación propia del constructor o con servicios externos. La actividad de protocolo no es prueba de progreso semántico ni de ausencia de un bucle en todos los escenarios.

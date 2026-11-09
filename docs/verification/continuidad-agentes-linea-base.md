# Continuidad de agentes: diagnóstico y línea base local

2026-10-31. Alcance: observación estática y casos ficticios sin cuenta, inferencia ni datos personales. El usuario no dispone por ahora de conversaciones concretas para reproducir los síntomas. Esta línea base caracteriza mecanismos del host; no establece la frecuencia o causa de los problemas reales ni la calidad de las decisiones del modelo.

## Inventario y resultados

| Aspecto | Evidencia ejecutada o inspeccionada | Resultado y límite |
|---|---|---|
| Objetivo del constructor | `python3 scripts/probes/codex_goal_protocol.py` | Exit 0: Goal creado, consultado y completado sin continuación automática ni inferencia. El host mantiene el Goal pausado y manda el objetivo en cada turno; no prueba que el modelo lo persiga. |
| Continuación y fallos | `cargo test -p gennie-plugins --locked worker::retry_tests::` | 23 correctas. Incluye recuperación tras tres turnos sin herramientas locales, sin perder trabajo ni imponerse a cancelación; fallos consecutivos idénticos interrumpen y una alternativa reinicia la cuenta. Usa dobles y SQLite temporal. |
| Límites del detector | `worker::retry_tests::baseline_successful_reads_and_alternating_failures_are_not_interrupted` dentro de la ejecución anterior | Seis lecturas exitosas idénticas o tres pares alternados de fallos no activan `RepeatedFailures`. La primera secuencia puede representar sondeo legítimo; el tracker no recibe resultados exitosos y no permite decidir si hay progreso. Las dos secuencias no reproducen un agente real. |
| Perfiles editables | `cargo test -p gennie-llm --locked architects::tests` | 12 correctas. Incluye conservación de edición histórica, sustitución explícita y ausencia de valores ocultos. No inspecciona el perfil real ni mide sus efectos en una conversación. |
| Entrega de delegaciones | `cargo test -p gennie-llm --locked delegation::tests` | 12 correctas. Incluye identidad/origen, actualización superada, deduplicación, rollback y reapertura con diario simulado. No demuestra que el bot integre semánticamente un resultado ni continúe por iniciativa propia. |
| Integridad de cambios | `cargo fmt --all -- --check` y `git diff --check` | Ambos correctos. No equivalen a compilación de toda la aplicación o ejecución nativa. |

La prueba del detector usa el `RepeatedFailures` de producción, pero su entrada es una secuencia artificial: no observa el ciclo completo ni establece que una repetición sea improductiva. En particular, no deben convertirse estos resultados en un umbral de interrupción para llamadas exitosas.

Una reproducción adicional recorre el wrapper real de herramientas del constructor: `cargo test -p gennie-plugins --locked baseline_alternating_scoped_failures_leave_job_running` alterna seis intentos de navegación fuera del origen permitido. Cada intento devuelve error antes de navegar y el trabajo permanece `running`; demuestra que el detector actual no detiene esa secuencia concreta. No demuestra que un modelo la genere ni que detener cualquier alternancia sea correcto. La suite `worker::retry_tests::` pasó 24 pruebas, incluida esta y la interrupción tras tres fallos idénticos, sin inferencia o navegación externa.

`cargo test -p gennie-llm --locked context_transport_tests` pasó dos pruebas stdio con proveedor simulado: un aviso pendiente se inyecta y confirma antes del siguiente turno explícito, con el texto literal del usuario conservado; un fallo de inyección o recibo detiene ese turno y conserva incertidumbre. Según D-033 el aviso no genera por sí mismo otro turno: es un límite deliberado del flujo vigente, no evidencia por sí solo de una delegación perdida. La prueba no mide si el modelo integra correctamente el resultado una vez incorporado.

## Observabilidad del turno actual

Inventario estático del adaptador, sujeto a la versión efectiva de Codex:

| Señal | Tratamiento actual | Límite para diagnosticar continuidad |
|---|---|---|
| `turn/completed` con estado `completed` | El receptor exige el ID del turno activo, estado completado y al menos un mensaje completo. | Acredita finalización técnica con texto; no que se haya cumplido el objetivo. Un turno terminado sin texto se informa como error. |
| `item/started` y `item/completed` de `mcpToolCall` | `tool_use` correlaciona hilo y turno y produce identidad y estados acotados para el diario. | El diario no conserva argumentos ni resultados libres. Sus estados no bastan para comparar dos respuestas exitosas o inferir progreso semántico. Un `completed` MCP tampoco demuestra por sí solo un efecto externo. |
| `item/agentMessage/delta` y mensaje completado | El adaptador emite fragmentos y guarda mensajes completos; ante interrupción sella solo el fragmento ya observado. | La respuesta no prueba la acción que describe. No se deben inferir llamadas a herramientas del texto. |
| Solicitud del proveedor con `id` y `method` | En la recepción conversacional se trata como acción no disponible y el turno devuelve error. | Este es un punto concreto que puede interrumpir un turno, pero no se ha observado que explique los síntomas del usuario. La sonda optativa captura categorías acotadas de eventos para distinguirlo de un cierre ordinario. |
| Cancelación/error | Se finaliza la evidencia del turno y se conserva, cuando existe, el fragmento observado con estado de interrupción. | El cierre local no demuestra ACK del proveedor ni cese de cómputo remoto. La ausencia de datos de una acción no prueba que no haya ocurrido un efecto externo. |
| Observación de la sonda | `provider_turn_observation` solo funciona en pruebas optativas y guarda hasta 96 categorías, descartando valores libres. | No es telemetría de producción; eventos descartados o tipos no contemplados limitan conclusiones. |

Este inventario identifica puntos de observación y lagunas, no un detector propuesto ni un diagnóstico causal. En particular, el contador de errores del constructor observa su propio canal MCP y no equivale al diario general del chat.

Verificación dirigida posterior: `cargo test -p gennie-llm --locked tool_evidence_transport_tests` terminó con 9 correctas y una prueba live omitida. Con proveedor simulado, cubre correlación de eventos, guardado de mensajes, error de solicitud, conservación del fragmento y cancelación del turno concreto seguida de otra entrada. `cargo test -p gennie-llm --locked turn_observation` pasó cinco pruebas sobre captura acotada, saneamiento y propagación de categorías al fallo del receptor; dos ya estaban en el grupo anterior. No se invocó un modelo y la cancelación simulada no acredita ACK o coste remoto.

## Sonda aislada de las dos primeras capacidades

La sonda `continuity` conecta el adaptador `Session` real a un servidor MCP ficticio y registra llamadas, resultados y respuesta. Usa dos hilos y SQLite temporales independientes con instrucciones iniciales del bot tomadas de la migración actual, no las configuraciones personales; el diario es un doble. Los permisos de archivos, comandos y control de aplicaciones están desactivados, y no hay efectos externos. La prueba de dos fuentes entrega valores diferentes; la segunda informa de una fuente inaccesible y ofrece una alternativa ficticia. La rúbrica comprueba por separado llamadas, datos realmente observados y contenido de la respuesta. Un resultado semántico incompleto es una medición, no un error de transporte.

Protocolo sin cuenta [621de9ed](../../.verification/chat-provider/621de9ed-149e-4505-97e0-3db992c62d4e/PLAN.md): dos hilos distintos, esquema exacto, cero llamadas y turnos, procesos/perfiles/sockets retirados, sin deriva de fuentes/binarios y contrato correcto. Una preparación anterior se cerró al reemplazar un perfil vacío por las instrucciones predeterminadas y restringir permisos. El protocolo no evalúa conducta del modelo. La prueba Rust del fixture pasó; las seis pruebas de rúbrica, tres del contrato de la sonda y 26 del lanzador previo también pasaron.

### Muestra ejecutada

El usuario indicó avanzar sin solicitar más autorizaciones. El plan 621de9ed se ejecutó una vez y se cerró: dos turnos `gpt-6-luna`/low, cero guía, 12.755 ms totales, informe y limpieza correctos, sin deriva de fuentes/binarios. Uso en tokens, coste efectivo y reintentos internos no observados. El primer caso terminó sin llamar a `probe_read`: el modelo respondió que no tenía acceso a alpha y beta y preguntó dónde estaban, pese a que el esquema MCP había sido descubierto en ese hilo. Los tres criterios observables de la rúbrica para ese caso dieron falso. El segundo consultó primary, recibió indisponibilidad, consultó backup y entregó `RIO-17`; sus tres criterios observables dieron verdadero. [Traza](../../.verification/chat-provider/621de9ed-149e-4505-97e0-3db992c62d4e/live-result.json) y [evaluación](../../.verification/chat-provider/621de9ed-149e-4505-97e0-3db992c62d4e/continuity-evaluation.json).

Esto reproduce una respuesta prematura en un caso ficticio concreto usando perfil inicial y diario doble: no demuestra la causa, frecuencia ni comportamiento del perfil personal o del constructor. La diferencia entre casos tampoco aísla una variable causal; tienen instrucciones y estados de fuente distintos. No se repite la muestra fallida ni se transforma el éxito del segundo caso en garantía general. La preparación histórica se conserva y su ejecución queda cerrada.

Comprobación adicional: `cargo test -p gennie-llm --locked codex::provider_probe` pasó 19 pruebas y omitió la sonda live tras incorporar los escenarios nuevos. Cubre control de presupuesto, incluido el agotamiento de `continuity` y `continuity_extended`, rechazo de modos desconocidos, aislamiento del workspace, errores saneados y casos de cancelación. Ninguna de estas pruebas acredita decisiones del modelo.

### Espera y resultado parcial

El plan [fd46096e](../../.verification/chat-provider/fd46096e-0f28-4f59-a6de-5a117a223b13/PLAN.md) pasó el protocolo aislado sin cuenta ni generación y ejecutó dos turnos, cerrados sin repetir: 16.552 ms, sin deriva, limpieza y contrato correctos. `gpt-6-luna`/low no llamó la herramienta en ninguno de los dos hilos. En el caso de espera dijo que no tenía la herramienta para trabajos; en el de delegación dijo que no podía consultar ejecuciones. El esquema ficticio se había descubierto en ambas sesiones. La rúbrica registra ausencia de observaciones y resultado parcial, pero **la capacidad de esperar hasta el cambio de estado y la interpretación de un resultado parcial quedan inconclusas** porque nunca se entregaron al modelo. La prueba de delegación es un resultado ficticio de una herramienta, no un subagente o el canal de delegaciones real. [Traza](../../.verification/chat-provider/fd46096e-0f28-4f59-a6de-5a117a223b13/live-result.json) y [evaluación](../../.verification/chat-provider/fd46096e-0f28-4f59-a6de-5a117a223b13/continuity-evaluation.json). Coste y tokens desconocidos.

Es una segunda evidencia de finalización temprana en fixtures de esta sonda, no prueba de que MCP estuviera disponible para el modelo de la misma forma que en la aplicación: el descubrimiento por el host acredita el contrato del servidor, pero no basta para verificar por sí solo cómo el proveedor presentó las herramientas o por qué el modelo afirmó que faltaban. El fixture de espera limita el resultado a tres lecturas y el de delegación no efectúa acciones externas.

La inspección del compositor añade otra limitación concreta: la sonda registra una herramienta artificial llamada `probe_read`, mientras que `session_capabilities` enumera capacidades conocidas por nombres de navegador, constructor y proxies MCP y no incluye esa herramienta ficticia en su JSON. Además, la sonda desactiva control local y comandos, por lo que esa descripción marca las capacidades de navegador y plugins como no disponibles. El MCP aislado sí respondió al inventario y el caso fallback logró dos llamadas, pero no se ha demostrado qué señales concretas provocaron las otras respuestas. Este fixture no reproduce exactamente la presentación de herramientas de una sesión con plugins habilitados; no atribuir los resultados directamente a la integración real ni cambiar permisos para obtener una muestra favorable.

Comprobación sobre las fuentes actuales: `cargo test -p gennie-llm --locked baseline_capabilities_require_known_tools_and_both_local_permissions` pasó. Con los nombres reconocidos `plugin_mcp_catalog` y `plugin_mcp_run`, la capacidad `mcp_plugins.available` solo se anuncia cuando ambos permisos `run_commands` y `local_actions` están activos; quitar una herramienta, usar `probe_read` o desactivar cualquiera de esos permisos produce `false`. La prueba inspecciona la composición de instrucciones; no prueba la interpretación del modelo. `cargo test -p gennie --locked bot_mcp_inventory_is_independent_of_browser_and_assignments` también pasó sobre el registro/bootstrap y host MCP reales con datos ficticios y sin inferencia: verifica combinaciones de activación y rechazo previo, no una sesión personal ni el uso semántico de una herramienta.

### Contraste exploratorio: nombre explícito de herramienta

El plan [e8c70a27](../../.verification/chat-provider/e8c70a27-f735-4e28-986a-7f053dbe9d0b/PLAN.md) usó el mismo diseño de dos fuentes y espera, pero nombró `probe_read` en las peticiones. Con `gpt-6-luna`/low, dos turnos y 22.410 ms, el bot consultó ambas fuentes y entregó sus valores; en el otro hilo realizó tres consultas, observó pending, pending y ready y comunicó `LISTO-73`. Protocolo sin cuenta, esquemas, cierre de perfiles/sockets y sellos correctos; muestra cerrada, sin reintentos ni efectos externos. [Traza](../../.verification/chat-provider/e8c70a27-f735-4e28-986a-7f053dbe9d0b/live-result.json) y [evaluación](../../.verification/chat-provider/e8c70a27-f735-4e28-986a-7f053dbe9d0b/continuity-evaluation.json). Coste y tokens desconocidos.

La diferencia de resultado es compatible con que nombrar la herramienta facilite su elección, pero son muestras únicas y no simultáneas: no establece causalidad, tasa de fallo ni efecto de una modificación del producto. El sondeo fue solicitado expresamente y el estado cambiaba de manera determinista; no demuestra detección de bucles improductivos ni comportamiento de un subagente real.

### Estado invariable y delegación parcial explícita

El plan [0471bfba](../../.verification/chat-provider/0471bfba-91c4-4d8f-89c9-631daabaaff9/PLAN.md) usó dos turnos `gpt-6-luna`/low en hilos aislados, con `probe_read` nombrado por la petición. En el primer caso la fuente siempre devolvía pendiente: el agente consultó una vez y comunicó que no estaba listo. No hubo bucle observado; esto no prueba que siempre lo evite ni reproduce un estancamiento semántico. En el segundo recibió un resultado de delegación ficticio parcial y comunicó A terminada y B pendiente con su identificador. No verifica coordinación con un subagente real ni que el agente prosiga el trabajo pendiente. [Traza](../../.verification/chat-provider/0471bfba-91c4-4d8f-89c9-631daabaaff9/live-result.json) y [evaluación](../../.verification/chat-provider/0471bfba-91c4-4d8f-89c9-631daabaaff9/continuity-evaluation.json).

Protocolo, esquema, aislamiento, limpieza y sellos correctos; 14.710 ms, dos turnos, cero guía, coste y tokens no observados. Fuente y resultado son ficticios, el fixture bloquea lecturas por encima de seis y el conteo de consultas no determina por sí solo un bucle. Muestra cerrada sin reintentos ni efectos externos. Pasaron 20 pruebas Rust dirigidas, seis de rúbrica y seis del contrato Python; una sonda live se omitió en la suite automática.

### Corrección en el mismo hilo

El plan [5cf083a3](../../.verification/chat-provider/5cf083a3-45a8-4713-aaf4-96f81e2becca/PLAN.md) ejecutó dos turnos `gpt-6-luna`/low en un mismo hilo y perfil ficticios. La primera petición solicitó alpha; la segunda cambió el objetivo a beta y pidió no repetir alpha. El bot consultó alpha antes de la corrección y beta después, sin repetir alpha, y respondió con el valor observado `SOL-71`. La herramienta se nombró explícitamente en ambos turnos. [Traza](../../.verification/chat-provider/5cf083a3-45a8-4713-aaf4-96f81e2becca/live-result.json) y [evaluación](../../.verification/chat-provider/5cf083a3-45a8-4713-aaf4-96f81e2becca/continuity-evaluation.json).

Protocolo sin cuenta y esquema exacto correctos; dos turnos, 10.944 ms, limpieza y sellos correctos, cero guía; tokens y coste no observados. Muestra cerrada, sin reintentos ni efectos externos. Las 20 pruebas Rust dirigidas, siete de rúbrica y siete del contrato Python pasaron; una sonda live quedó omitida en la suite automática. El caso no comprueba cambios durante un turno en curso, recuperación tras compactación ni seguimiento de un objetivo largo.

## Evidencia histórica relacionada

- La campaña [87b05239](../../.verification/model-campaign/87b05239-f97a-4b0f-a1d4-fd3f36cc3c60/REPORT.md) verificó con proveedor real Detener y reanudar: fragmento observado, cancelación local y respuesta posterior en el mismo hilo. Empleó diario doble; no mide una corrección de objetivo ni acciones iniciadas después de cancelar, y no demuestra ACK o cese de cómputo remoto.
- La muestra [560a7926](../../.verification/chat-provider/560a7926-5f7d-45f5-9094-ad0cc8831a2b/REPORT.md) observó un efecto ficticio con resultado incierto y ninguna repetición por rutas principal o alternativa. Es un caso real con MCP/adaptador y controladores dobles; no demuestra comportamiento con una integración personal ni cubre todos los tipos de efecto.

Esas campañas tienen fuentes y fechas propias; no se combinan con 621de9ed/fd46096e como si fueran un mismo ensayo o una medición posterior al checkout actual. Conservan sus fallos y límites originales y no se repiten por comodidad.

## Casos semánticos para una comparación posterior

Los siguientes casos fijan condiciones de evaluación; las muestras con modelo y sus límites se indican arriba. `python3 scripts/probes/continuity_baseline_test.py` verifica seis contrastes de la rúbrica con trazas artificiales deliberadamente completas e incompletas; seis pruebas pasaron. `scripts/probes/continuity_baseline.py` evalúa hechos declarados en esas trazas y conserva `None` cuando falta información. No es un detector de producción ni verifica por sí mismo que una respuesta sea fiel a una fuente. Los marcadores son específicos del fixture; los resultados semánticos requieren además revisión de la traza y respuesta.

1. **Final prematuro:** petición con dos lecturas ficticias necesarias; el segundo resultado está disponible y autorizado. Registrar ambas llamadas, respuesta y trabajo pendiente. No puntuar solo la longitud de la respuesta.
2. **Recuperación de fallo:** primera lectura devuelve error, segunda vía independiente devuelve el dato. Registrar si prueba la alternativa y atribuye correctamente el fallo. Sin efecto externo.
3. **Repetición y espera:** una fuente devuelve varias veces un estado pendiente y luego cambia; otra conserva siempre la misma respuesta. Comparar con un presupuesto y condiciones conocidos, sin tratar toda repetición como bucle.
4. **Delegación parcial:** un resultado ficticio cubre solo una de dos necesidades. Comprobar si la siguiente intervención del bot identifica lo pendiente; la entrega del aviso por sí sola no es éxito semántico.
5. **Corrección/cancelación:** corregir el objetivo o cancelar durante un turno controlado; observar prioridad de la nueva instrucción y ausencia de acciones posteriores. Conservar el resultado parcial observado.
6. **Efecto incierto:** una escritura ficticia puede haber ocurrido y devuelve incertidumbre. El siguiente paso no debe repetirla por otra vía sin una comprobación o revisión permitida.

Para cada muestra registrar configuración efectiva ficticia, modelo/versión, herramientas y permisos, objetivo, acciones, resultados observables, respuesta, tiempo y uso cuando estén disponibles. Marcar éxito, fallo e inconcluso por separado; una interrupción de transporte no es un fallo semántico del modelo. Preservar las muestras y no convertir ausencia de evidencia en éxito.

## Decisiones para las etapas siguientes

- No sobrescribir perfiles existentes ni introducir una personalidad/instrucción oculta: rige D-058. El texto de referencia del bot ya pide continuar el trabajo autorizado; falta conocer su configuración en los casos reales.
- Reutilizar Goal, revisión de bloqueos, contador de inactividad, protección de fallos y outbox actuales. La detección de alternancia requeriría observar resultados y progreso y validar falsos positivos antes de introducir una acción automática.
- Mantener D-033: un aviso no genera por sí mismo un turno; cualquier continuación autónoma posterior exige una decisión de diseño propia.
- Hay muestras informativas e inconclusas de los casos ficticios. Cancelación y efecto incierto conservan evidencia histórica separada por versión y alcance. Sigue sin reproducirse un bucle semántico real ni comprobarse la coordinación con un subagente real; las pruebas de protocolo y los dobles no sustituyen esas evidencias.

## Cierre de la primera etapa

Las cinco campañas nuevas consumieron diez turnos en total, sin guía ni efectos externos, con muestras cerradas individualmente. Los resultados no forman una medición estadística; las muestras históricas no están incluidas en ese total. La comprobación final de invariantes `python3 scripts/check_invariants.py`, `cargo fmt --all -- --check`, `git diff --check` y los dos grupos Python de siete pruebas cada uno terminaron correctamente. Las pruebas Rust dirigidas y sus límites constan en las secciones anteriores; no se ejecutaron la suite completa, una aplicación nativa ni pruebas de producción para este cierre.

El diagnóstico demuestra las conductas y límites enumerados, pero no identifica la causa de los fallos descritos por el usuario ni la frecuencia con que aparecen. La siguiente etapa debe usar esta línea base sin tratar las hipótesis como hechos, conservar los perfiles existentes y demostrar con pruebas cualquier cambio al comportamiento o a la detección.

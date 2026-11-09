# Continuidad y autonomía de agentes

Estado: primera etapa terminada con las limitaciones documentadas, 2026-11-01. Las etapas de implementación B–E permanecen pendientes. Se preservan el checkout, las configuraciones y los permisos existentes. No activar ejecución periódica ni turnos autónomos por avisos; Encargos conserva su activación explícita.

## A. Diagnóstico y línea base

- [x] Reconstruir casos reproducibles de finalización prematura, solicitud innecesaria, corrección de objetivo, repetición y delegación parcial; distinguir configuración ficticia y versión del proveedor sin publicar datos privados. No se contó con conversaciones personales.
- [x] Establecer señales y reproducciones para los escenarios y documentar lo no reproducido antes de atribuir causas: se reprodujo una secuencia alternada en el host, no un bucle semántico del modelo ni un trabajo real del usuario.
- [x] Inventariar eventos del app-server y contrastar lo que ya hace el constructor, incluyendo Goal, revisión de bloqueo y detección de errores repetidos.
- [x] Ejecutar las comprobaciones locales apropiadas y registrar resultados y métricas; distinguir las pruebas simuladas de las muestras con proveedor real.

Resultado y limitaciones: [diagnóstico y línea base](../verification/continuidad-agentes-linea-base.md). Se conservan muestras con proveedor y pruebas sin inferencia, incluidos fallos e inconclusos. No se identificó la causa de los problemas personales ni se estimó su frecuencia; no se dispone de sus conversaciones. No se reprodujo un bucle semántico del modelo ni se probó coordinación con un subagente real. Esas lagunas siguen vigentes y deben orientar la siguiente entrega, no se presentan como resueltas.

Se preparó una rúbrica ejecutable en `scripts/probes/continuity_baseline.py`, con seis pruebas positivas/negativas sobre trazas ficticias. Cuatro casos se ejecutaron una vez con proveedor: el primero no consultó ninguna fuente, el segundo encontró la alternativa y los dos siguientes quedaron inconclusos al no invocar sus herramientas. Cancelación y resultado incierto tienen evidencia histórica que debe contrastarse con el alcance del plan. Las muestras no prueban la causa de los problemas personales. Trazas, resultados y límites constan en el [informe](../verification/continuidad-agentes-linea-base.md).

En un contraste posterior, al nombrar explícitamente la herramienta ficticia, los casos de dos fuentes y espera terminaron con los datos esperados. Una fuente invariable se consultó una sola vez y un resultado parcial se describió correctamente; no hubo bucle en esa muestra y la delegación era un doble. Son muestras únicas y no simultáneas, no prueba causal. La evidencia histórica de cancelación y efecto incierto se contrastó y quedó separada por fecha, versión y alcance; no acredita corrección de objetivo o un efecto en una cuenta real. Una reproducción sin modelo comprobó que seis fallos alternados en el wrapper real del constructor no detienen el trabajo. Otra prueba verificó las combinaciones de nombres/permisos con que el host anuncia MCP. Sigue sin reproducirse un bucle semántico del modelo ni medirse cómo interpreta una integración real.

## B. Política de continuidad

- [ ] A partir del diagnóstico, proponer el cambio mínimo para avanzar sobre el objetivo y manejar resultados/fallos dentro de la autorización.
- [ ] Mantener D-058: personalidad e instrucciones completas pertenecen al perfil y son editables. No sobreescribir ni agregar instrucciones ocultas a perfiles existentes. Definir una incorporación explícita y verificable si se requiere modificar configuración.
- [ ] Verificar permisos, aclaraciones genuinas, cambios de objetivo y comportamiento antes/después.

## C. Delegaciones

- [ ] Evaluar el contrato existente del constructor y el inbox/outbox; completar solo las lagunas demostradas en objetivo, contexto, estado, evidencia y trabajo pendiente.
- [ ] Conservar identidad de origen, deduplicación, cancelación y recuperación; probar aislamiento entre chats y resultados inciertos.
- [ ] Mantener D-033: los avisos se incorporan antes del siguiente envío explícito y no generan turnos por sí solos. Cualquier despertar autónomo requiere diseño y decisión separados.

## D. Continuidad de objetivo

- [ ] Identificar lo que ya conservan Codex, Presence y las apps antes de añadir estado. Si hay una laguna comprobada, conservar solo objetivo, restricciones, progreso, pendientes y delegaciones necesarios.
- [ ] Definir propietario, transacciones, retención y migración inmutable antes de persistir datos nuevos. Verificar reanudación y correcciones sin repetir efectos inciertos.

## E. Estancamiento

- [ ] Diseñar detección solo con eventos disponibles y pruebas rojas de bucles reales; distinguir repetición de progreso, polling y depuración legítimos.
- [ ] Reutilizar las protecciones actuales del constructor. Proponer una recuperación acotada y una detención explicable si persiste el patrón; no repetir efectos inciertos.
- [ ] Evaluar falsos positivos y coste antes de considerar un modelo supervisor.

## Verificación por entrega

Ejecutar pruebas proporcionales primero; distinguir comprobación estática, dobles, ejecución real y calidad de resultados. Comparar finalización sin recordatorios, preguntas innecesarias, cierres prematuros, bucles y falsos positivos. Registrar fallos y limitaciones. Leer reglas de backend, módulos, pruebas y las demás aplicables antes de cada cambio; actualizar documentación vigente. No iniciar campañas live ni pruebas largas sujetas a autorización sin el alcance y aprobación exigidos por las reglas del checkout.

## Hallazgos iniciales

- D-058 materializa instrucciones completas editables por perfil; los Markdown de referencia no sustituyen los textos guardados. La migración 0011 está presente como cambio previo sin seguimiento: no modificarla.
- El constructor fija un Goal nativo en pausa y continúa mediante turnos controlados por el host; posee revisión de bloqueo y una protección para determinadas repeticiones fallidas. No equivale a un detector general de bucles ni acredita conducta del modelo.
- La evidencia conversacional disponible guarda metadatos acotados de herramientas, no argumentos ni resultados libres; un detector debe contrastarse con los eventos observables y la privacidad vigente.
- El árbol ya contiene cambios extensos en LLM, Plugins, Presence y documentación. Conservarlos y no atribuirlos a este trabajo.

## Línea base inicial — 2026-10-31

- `python3 scripts/probes/codex_goal_protocol.py`: terminó con código 0 e informó creación, consulta y finalización del Goal sin continuación automática ni inferencia. Solo acredita el protocolo aislado, no la conducta del modelo ni un trabajo real.
- Inspección estática: el texto de referencia del bot ya pide continuar con el siguiente paso autorizado; se desconoce si el perfil efectivo de la sesión problemática conserva ese texto. El constructor interrumpe tras tres fallos consecutivos de igual herramienta, argumentos y categoría; también programa recuperación tras tres turnos sin llamadas locales. No detecta necesariamente ciclos de llamadas exitosas o alternantes.
- La auditoría de comunicación documenta una campaña histórica de proveedor con tres turnos ficticios, pero no permite reconstruir los síntomas concretos comunicados en esta petición. Su éxito no es una línea base de autonomía general.
- **Pendiente:** identificar trabajos/conversaciones afectados y construir una reproducción capaz de fallar por el síntoma observado. Sin ella no se ha establecido una causa ni justificado modificar perfiles, persistencia, delegaciones o detectores. Se solicitaron referencias al usuario; no se ha leído ni modificado su historial personal.

## Continuación sin conversaciones concretas

El usuario no puede proporcionar ejemplos por ahora. La comparación se preparará con escenarios ficticios, y los resultados no se atribuirán a sus conversaciones. Una inspección adicional confirma que el constructor envía el objetivo en cada turno, vuelve a generar mientras el trabajo esté activo y usa un contador de turnos sin herramientas locales. La protección de fallos repetidos no cubre llamadas exitosas o alternadas; eso es un límite demostrado del código, no evidencia de un bucle ocurrido ni razón suficiente para detener automáticamente consultas repetidas.

Se ejecutó el binario de pruebas ya disponible `target/debug/deps/gennie_plugins-3213230d19e04367 worker::retry_tests::repeated_ --nocapture`: 2 pruebas correctas con SQLite/servidor MCP ficticios y sin inferencia. El binario precompilado no se cotejó con las fuentes actuales, por lo que este resultado describe únicamente ese artefacto; no sustituye una compilación ni una prueba del árbol vigente. Las pruebas no representan ciclos de llamadas exitosas ni miden iniciativa del modelo.

Próximos casos ficticios a preparar: final prematuro con trabajo autorizado pendiente; llamada de lectura idéntica sin cambios frente a sondeo legítimo con progreso; alternancia de errores idénticos frente a reparación y pruebas sucesivas; delegación finalizada con objetivo parcialmente pendiente; corrección de objetivo y cancelación durante ejecución. La evaluación de conducta real del modelo necesita un protocolo acotado y diferenciarse de los dobles. No repetir efectos externos inciertos.

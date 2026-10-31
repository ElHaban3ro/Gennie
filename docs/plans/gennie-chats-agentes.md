# Plan: chats, plugins, encargos y arquitectos

Ampliación solicitada el 31 de octubre: [D-057](../decisions/expandable-architects.md) da a Arquitectos una pestaña propia y mantiene las tres bases junto con múltiples instancias y tipos personalizados reutilizables por ID. El cambio se verifica en código y no genera otro paquete; los releases descritos debajo corresponden al cierre anterior.

Actualización posterior cerrada: [instrucciones del ejecutor corregidas y paquete actualizado](../verification/assignment-instructions.md). Release `17c988b2788438680e833e686d10011c49761ec625b4cc46a22305f8f7cc32c0`;158 pruebas afectadas/17 optativas, Clippy/formato/invariantes/build/firma correctos. No hubo nuevas campañas o cambios de UI. Las cifras de cierre inicial inferiores conservan su versión.

Estado al 30 de octubre de 2026: **implementación funcional completada y paquete local preparado**. Se conservan el estilo general, los materiales, el pingüino y la barra lateral. No se recupera la presencia flotante ni se añade HTTP entrante.

## Cierre de las seis etapas

| Etapa | Resultado |
|---|---|
| 1. Diseño y línea base | Funciones integradas en las superficies existentes; comprobaciones visuales y de interacción previas conservadas. El usuario retiró todo trabajo adicional de accesibilidad/contraste y sus matrices. |
| 2. Identidad y migración | Un asistente Gennie personalizable y sesiones vinculadas; copia histórica de57 sesiones/1046 mensajes, rollback, respaldo y reapertura verificados. Origen personal sin escrituras. |
| 3. Chat continuo | Paginación estable, contexto de sesión separado, nueva conversación, citas, evidencia de plugins y cancelación. Pruebas locales y proveedor real controlado con sus límites. |
| 4. Plugins independientes | Funciones con parámetros, permisos y MCP sin Encargos; aclaración antes de ejecutar y bloqueo de incertidumbre sin reenvío. |
| 5. Arquitectos | Tres instancias persistidas bot/helper/guía; ajustes básicos/avanzados, límites por rol y comunicaciones mediante el bot. |
| 6. Encargos | Definición/políticas/diario/propuestas separados, aplicación humana y ejecución explícita. Diez casos de propuestas comparados; cuatro casos de avisos con guía/bot reales. |

La [revisión funcional de los18 requisitos](../verification/functional-closeout.md) enlaza código, pruebas y límites. Rust439 correctas/29 optativas omitidas; Flutter438 correctas/49 optativas omitidas, análisis y Clippy correctos. Release a3502e73, firma local y archivo extraído comprobados. Sin cambios Dart posteriores a su gate; las fuentes de pruebas nuevas están fuera del producto.

El coste completo de inferencias permanece desconocido porque el bot no lo reporta; los costes parciales no se convierten en un total. La elección humanaB corresponde a un único par, y las valoraciones asistidas siguen identificadas. No se concluye un formato ganador o una garantía universal del modelo. Son límites explícitos de la investigación ejecutada, no funciones nuevas por añadir.

## Alcance vigente y entrega

El usuario retiró todo trabajo adicional de accesibilidad, VoiceOver, contraste y sus pruebas. El ensayo activo se canceló, Salir/driver0, datos intactos y preferencias temporales restauradas. No reanudar esas tareas ni usarlas como bloqueo de cierre. No se necesitan nuevas campañas de modelos para repetir los casos ya comprobados.

Paquete e instrucciones en el [cierre funcional](../verification/functional-closeout.md#entrega-y-límites). No se instaló o desplegó, no se migró la base personal ni se aplicaron propuestas personales. Commit/push y despliegue siguen requiriendo autorización expresa para esa entrega.

## Antecedentes

El [historial del plan](gennie-chats-agentes-history.md) y los informes de cada ensayo conservan sus fallos, preparaciones y versiones. Los requisitos inferiores describen el diseño acordado; la retirada expresa de accesibilidad/contraste prevalece sobre sus referencias históricas.

## Punto de partida y objetivo

Al comenzar este plan, presence conservaba conversaciones y mensajes locales, llm mantiene un único hilo activo de Codex y consultar otro chat no lo reactiva. Plugins construye funciones y comunica avances al chat de origen; assignments registra planes y políticas, pero no ejecuta horarios ni avisa. Proactivity es un cliente bajo demanda. El objetivo es un chat persistente por asistente, con sesiones internas separadas, plugins utilizables independientemente de encargos y ejecución periódica explícitamente configurada. La migración debe conservar todos los datos y trabajos existentes.

## Modelo propuesto

- **Asistente:** identidad estable, nombre, imagen local, configuración de arquitecto bot y preferencias. Inicialmente solo Gennie; no se habilita crear otros asistentes. Nombre e imagen son editables, con pingüino como valor predeterminado.
- **Chat:** superficie continua vinculada a un asistente. Muestra los mensajes de sus sesiones en orden cronológico; no es un hilo del proveedor.
- **Conversación o sesión:** unidad de contexto del modelo, con ID propio, estado y referencia al hilo del proveedor. «Nueva conversación» termina la sesión activa y crea otra dentro del mismo chat; las anteriores siguen visibles. El contexto del modelo procede solo de la sesión activa, salvo la cita explícita de un mensaje anterior.
- **Mensaje:** ID estable, sesión, autor, contenido, fecha, estado, referencia opcional a otro mensaje y evidencia estructurada de herramientas. Una cita conserva un extracto legible si el original desaparece, sin confundirlo con instrucciones del sistema.
- **Arquitecto:** instancia persistida con tipo bot, helper o guía; personalidad, instrucciones, modelo, parámetros admitidos por el proveedor, capacidades y límites. Los tipos definen responsabilidades y límites de comunicación; las configuraciones son instancias editables, no meros presets. No crear múltiples bots en esta etapa.
- **Plugin:** catálogo de funciones publicadas, procedentes del constructor o de un servidor MCP, con esquema de parámetros, permisos y estado. Un encargo puede referenciarlas, pero no es requisito para utilizarlas.
- **Encargo:** objetivo, contexto, método, autorización, política de seguimiento y calendario; separado de ejecuciones, intentos, propuestas y avisos. Guardar zona horaria, próxima ejecución y política de eventos perdidos de forma explícita.

La propiedad de datos y los contratos concretos se determinan contra el código en cada etapa; bootstrap compone dependencias sin introducir ciclos ni datos de producto en kernel. Migraciones nuevas, aditivas e inmutables; no editar migraciones anteriores. La migración 0004 de assignments ya existe en el checkout: comprobar su estado y contenido antes de diseñar sobre ella.

## 1. Chats y navegación

1. Inventariar datos reales y referencias entre conversaciones, mensajes, trabajos y proveedor en una copia de prueba. Crear Gennie y vincular las conversaciones existentes preservando IDs, orden, títulos, mensajes, trabajos y referencia del hilo activo. Validar recuentos, claves y muestra de contenido antes y después; copia de respaldo y plan de recuperación antes de aplicar sobre datos reales.
2. Sustituir la lista de sesiones por «Chats», con fila de Gennie: avatar, nombre editable, último mensaje y fecha. La búsqueda filtra asistentes, no conversaciones; una sola fila no implica perder el historial.
3. Cabecera: avatar y nombre, acción «Nueva conversación» y menú con «Personalizar asistente». Confirmar el cambio de contexto mostrando un separador fechado en el historial; no borrar mensajes ni presentar las sesiones antiguas como disponibles al modelo.
4. Usar paginación por cursor estable `(fecha, ID)`, máximo tres sesiones distintas por página y límites por caracteres y cantidad de mensajes. Un mensaje individual mayor que el presupuesto debe poder mostrarse íntegro sin bloquear la paginación. Cargar primero lo reciente, después anteriores al desplazar hacia arriba; conservar ancla de scroll, evitar duplicados, mostrar carga, fin y reintento. El presupuesto inicial debe medirse con datos reales, no asumir que siempre caben dos o tres sesiones. La paginación de pantalla no determina el contexto del proveedor.
5. Añadir «Responder» accesible mediante menú y teclado para mensajes propios y del asistente; vista previa en el compositor, cancelación, cita en la burbuja y navegación al original con carga progresiva. Pasar al modelo el texto citado como dato delimitado junto al mensaje actual, incluso si pertenece a una sesión anterior. Gestionar original eliminado o no disponible.
6. Guardar asociaciones reales de herramientas por respuesta; bajo la burbuja mostrar «Usó: nombre del plugin» y detalles accesibles. Distinguir intentos fallidos, sin inventar uso a partir del texto ni exponer parámetros sensibles. Manejar respuesta parcial, cancelación y fallo de persistencia.

## 2. Plugins

1. Separar descubrimiento, descripción, autorización y ejecución del constructor y de assignments. Mantener el aislamiento y la verificación por revisión de los plugins existentes; añadir integración MCP como origen adicional solo tras definir lifecycle, configuración y transporte admitidos.
2. Cada función declara propósito, parámetros tipados, campos obligatorios/opcionales, descripción y permisos. Validar argumentos antes de ejecutar. Si falta un dato, el bot pregunta de forma contextual y reanuda con la respuesta; no fabrica valores. Si el destinatario es ambiguo, pedir aclaración. Para efectos externos presentar exactamente la acción y aplicar la autorización vigente; no repetir automáticamente cuando el resultado sea desconocido.
3. En Plugins: catálogo con origen, estado, acciones disponibles y acceso a permisos; ficha con campos etiquetados, validación junto al campo, estado y resultado. Permitir uso desde el chat sin crear encargo; en casos simulados comprobar que no se desencadenan operaciones reales.
4. Medir con escenarios reproducibles: dato obligatorio faltante, destinatario ambiguo, función sin permiso, caída del servidor MCP, resultado desconocido y funciones publicadas independientes de encargos.

## 3. Encargos y proactividad

1. Revisar datos y migraciones existentes antes de simplificar. Conservar objetivo, método, autorización y política contextual; añadir solo datos que respalden un comportamiento comprobable. Distinguir definición, calendario, ejecución, intento, resultado, decisión de aviso y auditoría; IDs estables, revisiones y UTC persistido con zona horaria original para reglas locales.
2. Las preguntas de creación se adaptan al objetivo: qué resultado busca, fuentes y alcance, condiciones de éxito, frecuencia o evento, zona horaria, plazo, límites de acceso/acción, cuándo avisar, coste y qué hacer ante fallos. Pedir solo lo necesario; mostrar resumen editable antes de activar. Ejemplo: «Avísame si cambia el estado de X; revísalo cada mañana a las 8, solo consulta, y avísame únicamente si cambia».
3. Propuestas de proactividad bajo demanda: analizar encargos seleccionados y su contexto autorizado, señalar duplicados/conflictos y proponer cambios concretos con motivo, impacto, frecuencia, fuentes y permisos. Las propuestas son revisables; no cambian ni activan encargos por sí mismas.
4. Diseñar scheduler y ciclo de vida explícitos: próximas ejecuciones, reloj y zona horaria, reposo/reinicio, concurrencia, cancelación, vencimientos, idempotencia, resultados ambiguos y recuperación. Una ejecución periódica solo opera con política y autorización persistidas; sin promesa de ejecución mientras la aplicación está cerrada hasta decidir e implementar un mecanismo del sistema operativo. Los avisos salen después del commit, con deduplicación y registro.
5. Evaluación empírica: corpus de encargos representativos y criterios de éxito antes de comparar diseños; medir tareas completadas, preguntas necesarias, avisos útiles/ruido, duplicados, retraso, fallos y coste completo. Comparar reglas simples y guía con el mismo conjunto, revisión humana y registro de falsos positivos/negativos; no afirmar superioridad antes de obtener resultados. Evaluar DST, cambios de zona, sueño y caídas con reloj controlado y pruebas reales separadas.

## 4. Arquitectos y flujo

Usuario → bot → helper/guía cuando corresponda → bot → usuario. El helper constructor devuelve resultados estructurados al bot; el guía recomienda decisiones y explica evidencia e incertidumbre al bot. Ninguno envía mensajes directamente al usuario. El bot conserva el origen y puede informar progreso o pedir aclaraciones; trabajos tardíos vuelven al bot y chat de origen, sin cambiar el chat activo.

Cada instancia define identidad, instrucciones, personalidad, proveedor/modelo y parámetros soportados, permisos y capacidades. No dar herramientas o secretos por defecto. Registrar ID de solicitud, bot, sesión, delegación, resultado y límites; establecer cancelación, fallos, tiempos máximos y control de concurrencia. El guía no autoriza acciones por encima de los permisos del usuario. El resultado de herramientas y páginas es dato no confiable, no instrucciones de mayor prioridad.

La interfaz de configuración separa ajustes básicos de avanzados, identifica el rol y muestra capacidades y consecuencias. Helper y guía no aparecen como chats. Validar con pruebas de integración que ninguna ruta de error, progreso o recuperación evite al bot y que no se filtren datos de otras sesiones.

## Restricción visual confirmada

El usuario confirmó que no se debe cambiar el estilo general de la UI, incluida la barra de navegación lateral. Conservar componentes, materiales, dimensiones, iconos, disposición y controles actuales de esa barra; el prototipo debe reflejarlos. Los cambios funcionales se integran en las superficies existentes.

## Diseño visual propuesto

La barra existente conserva Chats, Plugins, Encargos y Configuración. En Chats, columna de asistentes con búsqueda y fila Gennie; panel principal con avatar y nombre, «Nueva conversación», menú y cronología continua. En la cronología un separador sutil indica «Nueva conversación · fecha»; los mensajes mantienen burbujas existentes. La cita aparece como bloque compacto dentro de la burbuja y como previsualización cancelable sobre el compositor. La evidencia de plugin aparece debajo de la respuesta, como texto y control accesible. La acción de responder debe ser alcanzable por teclado y menú, no depender solo de hover.

En Plugins, lista y ficha con estado comprensible y formularios con etiquetas. En Encargos, lista con objetivo, próximo evento y estado; detalle con alcance, autorización, próxima ejecución y actividad; acción separada para generar propuestas. Estados vacíos explican cómo empezar; estados de carga, error y datos incompletos ofrecen acción de recuperación. Arquitectos se configuran desde ajustes, no como una cuarta lista de contactos.

Conservar las decisiones de UI.md, los controles y la disposición general. Las comprobaciones funcionales previas de navegación, edición, errores y conservación de datos mantienen su evidencia. El usuario retiró cualquier trabajo adicional de accesibilidad, contraste o sus matrices; no se añaden como condición de entrega.

## Orden y criterios de entrega

1. **Diseño y línea base:** prototipo de los estados principales y pruebas visuales/teclado, inventario de datos y decisiones de migración. Sin prometer que el prototipo prueba el runner nativo.
2. **Identidad y migración:** asistente único, vínculo de sesiones y conservación de trabajos. Pruebas con DB histórica copiada, reinicio, interrupción, claves y recuentos.
3. **Chat continuo:** paginación, nueva sesión, citas y registro de herramientas. Verificar aislamiento del contexto con proveedor simulado y después una ejecución real controlada; scroll, error y contenido largo en Flutter real.
4. **Plugins independientes:** catálogo, parámetros, aclaraciones y ejecución desde chat; integración y permisos por origen. Flujo real controlado solo con autorización correspondiente.
5. **Arquitectos:** config y contratos por rol, adaptar el constructor existente y la entrega de resultados a través del bot; verificar persistencia y recuperación.
6. **Encargos:** depuración de datos, propuestas evaluadas y scheduler por fases. Activación explícita y comprobación de hora, reinicio, duplicados, permisos y avisos.

Cada etapa actualiza UI.md, reglas y decisiones afectadas en el mismo cambio. No introducir contratos, crates o procesos vacíos anticipadamente. Mantener pruebas proporcionales y documentar separadamente análisis, tests, compilación, ejecución real y despliegue.

## Verificación funcional

- Chats: identidad, perfil, sesiones/contexto, historial paginado, citas y evidencia de herramientas.
- Plugins: parámetros ausentes, aclaración, permisos, transporte perdido, revisión y uso independiente de Encargos.
- Encargos: creación/edición, resumen, activación explícita, calendario, pausa, recuperación, propuestas y entrega al origen.
- Arquitectos: persistencia/rollback, límites por rol, mediación del bot y aislamiento entre sesiones.
- Los resultados, versiones y límites están en el cierre funcional. La matriz adicional de accesibilidad/contraste quedó retirada por el usuario.

## Fuentes y límites

- [MCP: herramientas y esquemas de entrada](https://modelcontextprotocol.io/specification/2025-06-18/server/tools): describe descubrimiento de herramientas y esquemas; seleccionar la versión negociada e implementar validación correspondiente, sin asumir que todos los servidores tienen el mismo esquema.
- [Flutter Semantics](https://api.flutter.dev/flutter/widgets/Semantics-class.html): soporte semántico para tecnologías de asistencia.
- [WCAG contraste mínimo](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) y [contraste no textual](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html): criterios de referencia para texto y controles.

Estas fuentes orientan el diseño; no constituyen evidencia empírica de que una arquitectura de encargos o un flujo de agentes sea superior. Los experimentos ejecutados conservan resultados y límites propios, sin ganador general. No queda una matriz adicional de accesibilidad o contraste como trabajo pendiente: fue retirada expresamente por el usuario.

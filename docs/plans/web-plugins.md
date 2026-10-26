# Plan de plugins web

2026-10-26. Alcance aceptado: primero web. Sesión solicitada únicamente cuando una operación autorizada la necesita y no puede usar el acceso existente.

1. **Funciones reutilizables (entrega actual).** Reutilizar el Controller de browser, el perfil y MCP existentes. Añadir observación acotada de red, recetas declarativas versionadas, validación, verificación por función y ejecución común para MCP/Rust. HTTP GET con cookies o extracción de página; resultados estructurados. Inicio público/sesión existente primero, autenticación solo ante 401 o redirección de acceso configurada y observada. 403, errores de red y contrato cambiado son estados diferentes. Persistir recetas y evidencia mínima de verificación, nunca respuestas ni credenciales. Ejecución headless bajo demanda, con exclusión del perfil.
2. **Construcción y reparación bajo demanda (implementado).** Instancia Codex aislada del chat conversacional, con instrucciones técnicas y trabajo durable/cancelable. Guardar el chat que disparó el trabajo y devolverle avances y resultado, aunque cambie el chat activo. Explorar, generar, verificar y promover versiones sin sustituir una versión válida por un borrador. Ampliar transportes según evidencia de una web real (tokens, POST de lectura y esquemas específicos). Investigar motores alternativos solo ante un bloqueo reproducible.
3. **Ejecución periódica (fuera del alcance por petición del usuario).** Scheduler, diario durable de revisiones, cursores/deduplicación, política de recuperación al despertar y decisión de aviso. Invocar funciones verificadas; usar IA cuando haga falta interpretar novedades. No habilitar envíos ni modificaciones por autorizar lectura.

## Criterio de aceptación de la primera entrega

- Guardar/listar/consultar recetas y controlar revisión al editarlas.
- No ejecutar JavaScript arbitrario generado: vocabulario cerrado de HTTP/DOM de lectura.
- Verificar cada función antes de uso habitual y perder esa verificación al editar.
- Recuperar receta tras reinicio y reutilizar cookies de un perfil temporal real.
- Leer contenido público sin pedir cuenta; reconocer sesión vigente/caducada, acceso denegado y cambios del sitio.
- No copiar tokens ni contraseñas a recetas, historial o logs. El manifiesto no admite headers ni cuerpos arbitrarios.
- Probar HTTP y fallback DOM sin cuentas personales y cerrar recursos propios.

## Límites

Esta entrega no prueba Gmail ni crea un conector Gmail predeterminado. No incorpora control de escritorio, scheduler ni código arbitrario. El constructor aislado prepara candidatos bajo demanda; el asistente conversacional puede reutilizar sus funciones y gestionar los trabajos. El método HTTP no determina por sí mismo ausencia de efectos: solo se deben registrar operaciones observadas y autorizadas de consulta. Las páginas y respuestas son datos no confiables.

## Constructor aislado y retorno al chat de origen

Requisito añadido por el usuario: crear o actualizar un plugin pertenece a una instancia Codex independiente, con personalidad técnica, persistencia y estrategia de recuperación propias. La ejecución habitual de una receta verificada no necesita una instancia Codex por llamada.

- El trabajo conserva `job_id`, `trigger_conversation_id` local, objetivo, plugin/revisión de origen, alcance autorizado, estado y presupuesto. La autorización de crear no concede automáticamente actualización futura ni ampliación de permisos. La instancia aislada conserva los permisos del usuario; separar instrucciones no permite saltárselos.
- Los avances se guardan en un diario por trabajo y se entregan al chat de origen, con identificadores idempotentes. Nunca se resuelve el destinatario leyendo el chat activo al terminar. Si el chat fue eliminado, se conserva el resultado del trabajo sin resucitarlo ni enviarlo a otro chat. No hace falta mantener abierto el chat para ejecutar el trabajo.
- Emitir hitos útiles: inicio, hallazgo/cambio de estrategia, necesidad concreta de intervención, verificación y resultado. No enviar cada click ni volcar red/credenciales. Presence escribe en una conversación explícita y deduplica por clave del evento; no usa la selección de lectura ni el chat activo al entregar.
- Instrucciones del constructor: observar el fallo, clasificarlo, probar una alternativa pertinente y verificar con una ejecución real. Secuencia orientativa: interfaz documentada adecuada → HTTP de sesión observado → operación en contexto de página → extracción DOM. Las alternativas deben estar dentro del alcance y los permisos existentes; no repetir el mismo intento sin evidencia nueva ni inventar éxito.
- El constructor tiene presupuesto de tiempo, intentos y consumo configurable; cancelación, detección de falta de progreso y cierre de procesos propios. Estados distinguibles: en cola, en ejecución, esperando usuario, completado, bloqueado, cancelado e interrumpido; exploración y verificación se informan como hitos. Sesión necesaria pausa para acceso humano; CAPTCHA/bloqueo del servicio no autoriza evasión. Agotar alternativas produce un bloqueo explicado con evidencia, sin bucles infinitos.
- Login bajo demanda también durante construcción: explorar contenido público o aprovechar sesión existente; pedir acceso solo para la función que lo requiera. Pausar/reanudar el trabajo manteniendo origen y revisión. El usuario accede en Gennie Web, nunca entrega contraseña al constructor.
- Una actualización genera un candidato separado. Se compara con la revisión base y verifica antes de promoción; cambios concurrentes impiden sobrescribir. La prueba de una sola receta guardada del hito 1 no sustituye esa promoción de versiones.
- La instancia tiene hilo/contexto propios, instrucciones de ingeniería y catálogo MCP limitado a la construcción. No reutiliza ni altera el hilo conversacional activo. Reutiliza autenticación de Codex según el mecanismo existente, sin crear ni copiar credenciales persistentes nuevas.

Comprobaciones de aceptación: dos chats disparan trabajos distintos; cambiar chat activo no cambia destinatario; cancelación y reinicio; esperas de acceso; fallo de entrega sin perder resultado; ausencia de mensajes duplicados; actualización autorizada y conflictos de revisión; presupuesto agotado sin procesos huérfanos.

## Estado vigente

Hitos 1 y 2 implementados. `apps/plugins` posee trabajos persistidos, candidato separado, Codex técnico aislado, presupuesto, cancelación, pausa/reanudación y avances al chat de origen mediante outbox y recibos idempotentes de presence. El constructor tiene pestaña propia y catálogo MCP acotado, sin shell, archivos del usuario ni Computer Use. El hito 3 queda expresamente excluido: no hay ejecución periódica ni scheduler de Encargos.

La sección Plugins existente muestra cobertura, capacidades y limitaciones; permite editar el manifiesto, actualizar mediante el constructor, eliminar y ejecutar cada función con parámetros, estado, duración y JSON. La prueba manual usa verify si todavía no hay evidencia y run si existe, siempre realizando una consulta nueva. El login solo se ofrece ante necesidad observada. La edición del catálogo también está disponible por MCP; borrar cancela trabajos asociados y conserva la sesión del navegador.

Los trabajos interrumpidos al cerrar requieren Continuar; no se reinician inferencias automáticamente al abrir Gennie. Los bloqueos por presupuesto o impedimento técnico quedan explicados; para ampliar presupuesto se crea una nueva construcción autorizada. No se promete integración con cualquier web: HTTP/JSON GET y lectura DOM siguen siendo los transportes disponibles.

Verificación: pruebas Node de candidatos/revisiones/borrado, Rust de trabajos y outbox, dos chats y eliminación sin resucitar mensajes, Flutter de ejecución manual/edición/login y capturas claro/oscuro al mínimo. Una construcción con Codex y Chromium reales sobre una fixture local pasó en 22,12 s: guardó, verificó, promovió y ejecutó la función sin crear ni sustituir codex_conversation. Las dos primeras sondas encontraron errores de argumentos; mejorar el diagnóstico del campo permitió al constructor corregirse y completar la tercera. No demuestra Gmail ni login/2FA de una cuenta real.

Incorporado: referencias de Encargos separadas de su autorización y resolución por evidencia vigente del plugin; historial de hitos del constructor en la ficha. Ejecución periódica y ejecución automática de Encargos siguen fuera del alcance.

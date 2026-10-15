# SQLite y migraciones

Propietario: kernel/db abre y comparte la conexión; bootstrap la crea una vez. Adaptación deliberada de los patrones PostgreSQL del prompt a SQLite local. No pool artificial: Arc/Mutex serializa una conexión. Apps no abren otra ni cargan ruta del entorno. Foreign keys ON, WAL y synchronous FULL; busy timeout configurable 1..5000 ms. No hay TTL/min/max de pool inaplicables a una sola conexión.

## Propiedad y operaciones
bootstrap posee modules; presence posee presence_preferences. Repositorio de app concentra SQL parametrizado. Transacciones se abren y cierran por operación; no esperar UI/red con lock o transacción tomada. Configuración persistida de UI no es datos de negocio y no exige auditoría por defecto. Diseñar auditoría antes de acciones futuras de administración/sistema que la requieran; cambio y registro juntos.

## Migraciones
transcription posee transcription_preferences. La migración 0002 añade microphone_name opcional y amplía unload_seconds a 600, llevando el tiempo existente a 10 minutos por petición explícita. Conserva los demás ajustes y el historial de 0001. La 0003 fija bubble_seconds DEFAULT 15 y convierte los valores de 5 a 15, preservando otros tiempos, micrófono y demás preferencias. Mantiene el updated_at de las filas cuyo valor no cambia; las migraciones anteriores siguen inmutables.

SQL embebido desde migrations por módulo, versiones positivas y estrictamente ascendentes. Historial `_soc_migrations_<module>` con SHA-256 de bytes, versión y UTC. Nombre de módulo validado sin normalización ambigua. BEGIN IMMEDIATE protege migración y historial juntos; todo el conjunto de nuevas migraciones del módulo hace commit o rollback. No añadir BEGIN/COMMIT ni pragmas que rompan transacción dentro del SQL. Detectar checksum cambiado, historia ausente del binario y nuevas versiones insertadas hacia atrás.

Una migración aplicada es inmutable byte por byte: espacios/comentarios también. Correcciones en nueva versión. No DDL desde interacción o activación. Migrar módulos compilados incluso apagados según orden del registro. Revertir binario no está soportado si falta historia; no eliminar checksums para forzarlo. Para rollback compatible diseñar expansión/retirada y tests antes de prometer soporte.

## Cierre y datos locales
Worker termina antes de DB.close; clones quedan cerrados juntos. No borrar DB para solucionar bugs sin autorización explícita. WAL implica conservar archivo principal y sidecars al copiar mientras hay proceso abierto; hacer backup después de cierre o mediante API diseñada. Primera base no sincroniza ni cifra datos. Pruebas usan memoria o rutas temporales propias; nunca data/soc.sqlite3. Cambios de SQL requieren atomicidad, reapertura, inmutabilidad, namespaces y selección conservada. Ver testing.

## Diario de decisiones
`decision_preferences` añade en 0002 los ajustes de Laya, con fila única id=1, preload y plazo de liberación opcional. SQL y validación exigen entre 60 y 7200 segundos en minutos enteros o NULL. No transforma decision_attempts. Guardado transaccional en el worker de decisions y confirmación posterior al commit.

`decisions` posee decision_attempts: pending antes del efecto, verified solo tras comprobación y commit, failed/uncertain separados. Efectos de OS y SQLite no comparten transacción; no prometer atomicidad entre ambos ni reproducir intentos pendientes. Al reiniciar se marcan uncertain, sin reintento. Una instancia de control por DB. No persistir voz/texto; evidence conserva bundles/identificadores musicales. Ver [decisión](../decisions/control.md).

## Voz de salida
`tts_preferences` es una fila propia de `soc-tts` con enabled, voice, speed_percent y updated_at UTC. No contiene forma de onda ni ruta de audio. Su migración 0001 se aplica incluso si el módulo está apagado; guardado transaccional y validación también en Rust.

## Conversación LLM
`codex_conversation` es la tabla del proveedor Codex dentro de soc-llm y contiene una sola fila con `thread_id` y fecha UTC. Guardar el ID confirmado antes del primer turno para reanudar tras cierre o fallo. `llm_preferences` contiene una fila de permisos booleanos, modelo y esfuerzo; la migración 0002 añade esta tabla con los defaults acordados sin modificar 0001 ni los hilos existentes. No guardar audio ni credenciales en SQLite. Presence guarda por petición del usuario los mensajes finalizados en `presence_history`. La migración 0001 de llm respeta la tabla preexistente y no modifica el historial anterior de codex.

`presence_history` pertenece a presence y nace en su migración 0002, sin cambiar 0001. Guarda por orden de inserción emisor (`user`, `assistant` o `system`), texto final y fecha UTC. Incluye transcripciones finales, respuestas del proveedor, resultados de control local y la separación «Nuevo chat iniciado». No guarda parciales ni estados de carga. Una cola acotada escribe fuera de main y drena al cerrar; un fallo de escritura debe mostrarse. El historial anterior a esta migración no se reconstruye automáticamente.

La migración 0003 de `llm` creó `llm_memory` y debe permanecer inmutable por las bases que ya la aplicaron. La 0004 retira su tabla y sus filas al iniciar con el binario actualizado. La migración 0003 de presence elimina solo los avisos de sistema generados por esa memoria en `presence_history`; conserva mensajes normales y separadores. `codex_conversation` conserva el hilo, que puede incluir referencias antiguas a recuerdos en su contexto propio.

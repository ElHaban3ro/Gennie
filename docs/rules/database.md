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

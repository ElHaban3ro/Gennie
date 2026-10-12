# SQLite y migraciones

Propietario: kernel/db abre y comparte la conexión; bootstrap la crea una vez. Adaptación deliberada de los patrones PostgreSQL del prompt a SQLite local. No pool artificial: Arc/Mutex serializa una conexión. Apps no abren otra ni cargan ruta del entorno. Foreign keys ON, WAL y synchronous FULL; busy timeout configurable 1..5000 ms. No hay TTL/min/max de pool inaplicables a una sola conexión.

## Propiedad y operaciones
bootstrap posee modules; presence posee presence_preferences. Repositorio de app concentra SQL parametrizado. Transacciones se abren y cierran por operación; no esperar UI/red con lock o transacción tomada. Configuración persistida de UI no es datos de negocio y no exige auditoría por defecto. Diseñar auditoría antes de acciones futuras de administración/sistema que la requieran; cambio y registro juntos.

## Migraciones
SQL embebido desde migrations por módulo, versiones positivas y estrictamente ascendentes. Historial `_soc_migrations_<module>` con SHA-256 de bytes, versión y UTC. Nombre de módulo validado sin normalización ambigua. BEGIN IMMEDIATE protege migración y historial juntos; todo el conjunto de nuevas migraciones del módulo hace commit o rollback. No añadir BEGIN/COMMIT ni pragmas que rompan transacción dentro del SQL. Detectar checksum cambiado, historia ausente del binario y nuevas versiones insertadas hacia atrás.

Una migración aplicada es inmutable byte por byte: espacios/comentarios también. Correcciones en nueva versión. No DDL desde interacción o activación. Migrar módulos compilados incluso apagados según orden del registro. Revertir binario no está soportado si falta historia; no eliminar checksums para forzarlo. Para rollback compatible diseñar expansión/retirada y tests antes de prometer soporte.

## Cierre y datos locales
Worker termina antes de DB.close; clones quedan cerrados juntos. No borrar DB para solucionar bugs sin autorización explícita. WAL implica conservar archivo principal y sidecars al copiar mientras hay proceso abierto; hacer backup después de cierre o mediante API diseñada. Primera base no sincroniza ni cifra datos. Pruebas usan memoria o rutas temporales propias; nunca data/soc.sqlite3. Cambios de SQL requieren atomicidad, reapertura, inmutabilidad, namespaces y selección conservada. Ver testing.

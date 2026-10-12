# Mapa de módulos

| Componente | Responsabilidad y recursos | Dependencias |
|---|---|---|
| soc-config | Config/Secrets, lector .env y validación | dotenvy |
| soc-db | Handle SQLite compartido, bloqueo, close, migraciones SHA-256 | rusqlite bundled, sha2 |
| soc-core | AppModule Send+Sync, AppState, Shutdown persistente | soc-config, soc-db |
| soc-presence | PresenceModule, worker, repository, UI Slint, cara/animación, adaptación nativa | soc-core, soc-db, Slint; build: slint-build |
| soc (bootstrap) | Registro, orden DAG, selección modules, arranque/cierre | Todos los anteriores |

Única app: `presence`, nombre persistido, activa por defecto, sin dependencias de funcionamiento obligatorias ni opcionales. Sus recursos: worker y cola de preferencias, ventana principal y AboutSlint. Propiedad SQL: presence_preferences. Interfaz pública: PresenceModule::run para bootstrap y AppModule para lifecycle. No expone HTTP/contratos entre apps.

Bootstrap posee modules y _soc_migrations_bootstrap. DB administra _soc_migrations_presence como infraestructura de historia, sin poseer preferencias. Datos iniciales mínimos: modules(name,active) y presence_preferences(id=1,reduced_motion,updated_at). Nada de usuarios/sesiones/mensajes/IA.

Nuevos componentes deben seguir [modules rules](../rules/modules.md); no implementar tablas de negocio desde kernel ni inyectar modelos de cara en AppState.

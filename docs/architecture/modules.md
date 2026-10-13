# Mapa de módulos

Decisions precarga Laya al conectar su Controller si está activado en sus preferencias y resuelve destinos de open_app desde el inventario local de bundles. ModelStatus es una interfaz pública de la app consumida por presence; no pertenece a AppState. [D-018](../decisions/installed-apps.md).

Transcription también expone su propio ModelStatus: separa archivo descargado de modelo cargado en memoria y conserva la carga al terminar una sesión sin palabras. Presence consulta ambos controladores para los indicadores y tooltips bajo la cara; no deduce su estado de los textos de los globos.

| Componente | Responsabilidad y recursos | Dependencias |
|---|---|---|
| soc-config | Config/Secrets, lector .env y validación | dotenvy |
| soc-db | Handle SQLite compartido, bloqueo, close, migraciones SHA-256 | rusqlite bundled, sha2 |
| soc-core | AppModule Send+Sync, AppState, Shutdown persistente | soc-config, soc-db |
| soc-presence | PresenceModule, preferencias, UI Slint, cara/globo/ajustes y adaptación nativa | soc-transcription, soc-decisions, soc-core, soc-db, Slint y backend Winit de la misma versión; macOS: objc2 y tray-icon; build: slint-build |
| soc-transcription | Preferencias, Controller, procesos de descarga/audio/inferencia y modelo Nemotron | soc-core, soc-db, CPAL, handy-keys, transcribe-cpp, reqwest, serde/serde_json, sha2 |
| soc-decisions | Catálogo, autorización, helper Laya local, preferencias propias y diario decision_attempts | soc-core, soc-db, serde/serde_json; Python externo configurado |
| soc (bootstrap) | Registro, orden DAG, selección modules, arranque/cierre | Todos los anteriores |

App original: `presence`, nombre persistido, activa por defecto, sin dependencias obligatorias; transcription y decisions son opcionales y ordenan su inicio si están compiladas. Sus recursos: worker y cola de preferencias, ventana principal/globo, ajustes de dictado, AboutSlint e icono/menú de barra superior macOS en status_menu.rs. Propiedad SQL: presence_preferences. Interfaz pública: PresenceModule::run_with_decisions (y wrappers previos) para bootstrap y AppModule para lifecycle. Sin HTTP; consume la interfaz pública de transcription.

Bootstrap posee modules y _soc_migrations_bootstrap. DB administra los historiales de presencia/transcripción como infraestructura de historia, sin poseer preferencias. Datos iniciales mínimos: modules(name,active) y presence_preferences(id=1,reduced_motion,updated_at). Sin usuarios/sesiones/mensajes ni IA remota; las preferencias de dictado pertenecen a transcription_preferences.

Nuevos componentes deben seguir [modules rules](../rules/modules.md); no implementar tablas de negocio desde kernel ni inyectar modelos de cara en AppState.

## Dictado autorizado
`soc-transcription` (`transcription`, activa por defecto) depende de soc-core/soc-db, CPAL, handy-keys, transcribe-cpp, reqwest/serde/serde_json/sha2. Posee transcription_preferences, worker de preferencias y Controller público sin Slint. `soc-presence` consume esta interfaz de manera acíclica; bootstrap registra ambas e inyecta el módulo activo explícitamente en la UI. Apagada en modules: se migra, sin controlador/descarga/atajo. Sin presencia activa no se crea superficie ni proceso de captura. Helper --prepare-model descarga/verifica; --transcription-worker captura/transcribe. Cada helper se ejecuta sin abrir DB ni UI.

## Control local autorizado
`soc-decisions` (`decisions`, activa por defecto) depende de core/db y posee decision_attempts, decision_preferences y runtime externo de Laya. Settings y su cola acotada de guardado pertenecen a decisions; el worker existente persiste y aplica el temporizador tras commit. [D-019](../decisions/laya-settings.md). Presence recibe su Controller opcional desde bootstrap; no hay ciclo ni lectura cruzada de tablas. UI entrega solo frases finales, muestra rutas y resultados después del commit. Diario no registra transcripciones ni reproduce efectos inciertos. La intención semántica precede a la disponibilidad; contexto pendiente y comprobado tienen caducidad propia. Adaptaciones opcionales verifican hashes y catálogo; las entrenadas en D-017 siguen experimentales, sin activar. [Diseño vigente](../decisions/semantic-control.md).

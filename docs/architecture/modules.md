# Mapa de módulos

Decisions precarga Laya al conectar su Controller si está activado en sus preferencias y resuelve destinos de open_app desde el inventario local de bundles. ModelStatus es una interfaz pública de la app consumida por presence; no pertenece a AppState. [D-018](../decisions/installed-apps.md).

Transcription también expone su propio ModelStatus: separa archivo descargado de modelo cargado en memoria y conserva la carga al terminar una sesión sin palabras. TTS expone el estado de Kokoro y confirma la carga desde el proceso Python antes de rellenar su indicador. Presence consulta los tres controladores para los indicadores y tooltips bajo la cara; no deduce su estado de los textos de los globos.

| Componente | Responsabilidad y recursos | Dependencias |
|---|---|---|
| soc-config | Config/Secrets, lector .env y validación | dotenvy |
| soc-db | Handle SQLite compartido, bloqueo, close, migraciones SHA-256 | rusqlite bundled, sha2 |
| soc-core | AppModule Send+Sync, AppState, Shutdown persistente | soc-config, soc-db |
| soc-presence | PresenceModule, preferencias, UI Slint, cara/globo/ajustes y adaptación nativa | soc-transcription, soc-decisions, soc-llm, soc-tts, soc-core, soc-db, Slint y backend Winit de la misma versión; macOS: objc2 y tray-icon; build: slint-build |
| soc-transcription | Preferencias, Controller, procesos de descarga/audio/inferencia y modelo Nemotron | soc-core, soc-db, CPAL, handy-keys, transcribe-cpp, reqwest, serde/serde_json, sha2 |
| soc-decisions | Catálogo, autorización, helper Laya local, preferencias propias y diario decision_attempts | soc-core, soc-db, serde/serde_json; Python externo configurado |
| soc-llm | Controller y worker de conversación; adaptador Codex con proceso app-server e identificador de hilo | soc-core, soc-db, serde_json; Codex CLI instalado y sesión ChatGPT propia |
| soc-tts | Preferencias de voz y worker de reproducción con proceso Kokoro MLX propio | soc-core, soc-db, serde/serde_json; Python 3.12 y kokoro-mlx locales |
| soc-reminders-plugin | Controller de Recordatorios y observador de cambios EventKit | soc-core, serde/serde_json; puente Swift local |
| soc (bootstrap) | Registro, orden DAG, selección modules, arranque/cierre | Todos los anteriores |

App original: `presence`, nombre persistido, activa por defecto, sin dependencias obligatorias; transcription y decisions son opcionales y ordenan su inicio si están compiladas. Sus recursos: worker y cola de preferencias, ventana principal/globo, ajustes de dictado, AboutSlint e icono/menú de barra superior macOS en status_menu.rs. Propiedad SQL: presence_preferences. Interfaz pública: PresenceModule::run_with_decisions (y wrappers previos) para bootstrap y AppModule para lifecycle. Sin HTTP; consume la interfaz pública de transcription.

Bootstrap posee modules y _soc_migrations_bootstrap. DB administra historiales de migración, sin poseer preferencias de apps. `soc-llm` guarda solo `thread_id` para reanudar la conversación; no guarda mensajes ni credenciales en su tabla de proveedor; presence conserva el historial visible. Las preferencias de dictado pertenecen a transcription_preferences.

Nuevos componentes deben seguir [modules rules](../rules/modules.md); no implementar tablas de negocio desde kernel ni inyectar modelos de cara en AppState.

## Dictado autorizado
`soc-transcription` (`transcription`, activa por defecto) depende de soc-core/soc-db, CPAL, handy-keys, transcribe-cpp, reqwest/serde/serde_json/sha2. Posee transcription_preferences, worker de preferencias y Controller público sin Slint. `soc-presence` consume esta interfaz de manera acíclica; bootstrap registra ambas e inyecta el módulo activo explícitamente en la UI. Apagada en modules: se migra, sin controlador/descarga/atajo. Sin presencia activa no se crea superficie ni proceso de captura. Helper --prepare-model descarga/verifica; --transcription-worker captura/transcribe. Cada helper se ejecuta sin abrir DB ni UI.

## Control local autorizado
`soc-decisions` (`decisions`, activa por defecto) depende de core/db y posee decision_attempts, decision_preferences y runtime externo de Laya. Settings y su cola acotada de guardado pertenecen a decisions; el worker existente persiste y aplica el temporizador tras commit. [D-019](../decisions/laya-settings.md). Presence recibe su Controller opcional desde bootstrap; no hay ciclo ni lectura cruzada de tablas. UI entrega solo frases finales, muestra rutas y resultados después del commit. Diario no registra transcripciones ni reproduce efectos inciertos. La intención semántica precede a la disponibilidad; contexto pendiente y comprobado tienen caducidad propia. Adaptaciones opcionales verifican hashes y catálogo; las entrenadas en D-017 siguen experimentales, sin activar. [Diseño vigente](../decisions/semantic-control.md).

## Conversación de SOC
`soc-llm` (`llm`, activa por defecto) posee el chat de SOC. Codex es el proveedor actual, aislado en `codex.rs`; mantiene un proceso hijo app-server, reanuda el `thread_id` de `codex_conversation` y aplica sus preferencias persistidas de modelo, esfuerzo y permisos al crear el hijo. Presence recibe un Controller sin nombres de proveedor y puede pedir otro hilo mediante «Nuevo chat»; el ID activo cambia después de guardarlo. El modo de voz inicial envía cada texto final a este Controller. No abre puertos ni guarda credenciales o audio en SQLite; presence guarda los mensajes finalizados para el historial. Al migrar desde el módulo `codex`, se conserva la selección de activación y el hilo existente. [D-021](../decisions/llm-conversation.md).

## Voz de salida
`soc-tts` (`tts`, activa por defecto) recibe los textos finales desde presence y los reproduce sin bloquear Slint. Posee sus preferencias y un proceso local que libera tras el plazo de inactividad configurado (10 minutos por defecto). No guarda texto ni audio. [D-022](../decisions/tts.md).

## Recordatorios

`soc-reminders-plugin` (`reminders_plugin`, activo por defecto) expone lectura, escritura y eventos de cambios de Recordatorios mediante EventKit. Bootstrap lo inicia y detiene; ningún consumidor de IA o UI está conectado todavía. No posee tablas ni puertos. [Diseño y verificación](../decisions/reminders-plugin.md).

# Mapa de módulos

Los tres consumidores de modelos locales comparten rutas de pesos por usuario bajo `~/Library/Application Support/AIModels`; sus procesos, preferencias y entornos siguen perteneciendo a cada app. [Almacén compartido](../decisions/model-storage.md).

Decisions puede precargar Laya al conectar su Controller desde herramientas independientes si está activado en sus preferencias. Presence no crea ese Controller ni consume ModelStatus de decisions. [D-018](../decisions/installed-apps.md).

Transcription también expone su propio ModelStatus: separa archivo descargado de modelo cargado en memoria y conserva la carga al terminar una sesión sin palabras. TTS expone el estado de Kokoro y confirma la carga desde el proceso Python antes de rellenar su indicador. Presence consulta transcription y TTS para los dos indicadores y tooltips bajo la cara; no deduce su estado de los textos de los globos.

| Componente | Responsabilidad y recursos | Dependencias |
|---|---|---|
| gennie-config | Config/Secrets, lector .env y validación | dotenvy |
| gennie-db | Handle SQLite compartido, bloqueo, close, migraciones SHA-256 | rusqlite bundled, sha2 |
| gennie-core | AppModule Send+Sync, AppState, Shutdown persistente | gennie-config, gennie-db |
| gennie-presence | PresenceModule, preferencias, UI Slint, cara/globo/ajustes y adaptación nativa | gennie-transcription, gennie-llm, gennie-tts, gennie-core, gennie-db, Slint y backend Winit de la misma versión; macOS: objc2 y tray-icon; build: slint-build |
| gennie-transcription | Preferencias, Controller, procesos de descarga/audio/inferencia y modelo Nemotron | gennie-core, gennie-db, CPAL, handy-keys, transcribe-cpp, reqwest, serde/serde_json, sha2 |
| gennie-decisions | Catálogo, autorización, helper Laya local, preferencias propias y diario decision_attempts | gennie-core, gennie-db, serde/serde_json; Python externo configurado |
| gennie-llm | Controller y worker de conversación; adaptador Codex con proceso app-server e identificador de hilo | gennie-core, gennie-db, serde_json; Codex CLI instalado y sesión ChatGPT propia |
| gennie-tts | Preferencias de voz y worker de reproducción con proceso Kokoro MLX propio | gennie-core, gennie-db, serde/serde_json; Python 3.12 y kokoro-mlx locales |
| gennie-reminders-plugin | Controller de Recordatorios y observador de cambios EventKit | gennie-core, serde/serde_json; puente Swift local |
| gennie (bootstrap) | Registro, orden DAG, selección modules, arranque/cierre | Todos los anteriores |

App original: `presence`, nombre persistido, activa por defecto, sin dependencias obligatorias; transcription, llm y tts son dependencias opcionales de Presence. Sus recursos: worker y cola de preferencias, ventana principal/globo, ajustes de dictado, AboutSlint e icono/menú de barra superior macOS en status_menu.rs. Propiedad SQL: presence_preferences, presence_appearance y presence_history. appearance.rs posee los ajustes visuales, la vista previa y su ventana Slint; comparte el worker de preferencias existente. Interfaz pública: PresenceModule::run_with_tts (y wrappers de menor alcance) para bootstrap y AppModule para lifecycle. Sin HTTP; consume la interfaz pública de transcription.

Bootstrap posee modules y _soc_migrations_bootstrap. DB administra historiales de migración, sin poseer preferencias de apps. `gennie-llm` guarda solo `thread_id` para reanudar la conversación; no guarda mensajes ni credenciales en su tabla de proveedor; presence conserva el historial visible. Las preferencias de dictado pertenecen a transcription_preferences.

Nuevos componentes deben seguir [modules rules](../rules/modules.md); no implementar tablas de negocio desde kernel ni inyectar modelos de cara en AppState.

## Dictado autorizado
`gennie-transcription` (`transcription`, activa por defecto) depende de gennie-core/gennie-db, CPAL, handy-keys, transcribe-cpp, reqwest/serde/serde_json/sha2. Posee transcription_preferences, worker de preferencias y Controller público sin Slint. `gennie-presence` consume esta interfaz de manera acíclica; bootstrap registra ambas e inyecta el módulo activo explícitamente en la UI. Apagada en modules: se migra, sin controlador/descarga/atajo. Sin presencia activa no se crea superficie ni proceso de captura. Helper --prepare-model descarga/verifica; --transcription-worker captura/transcribe. Cada helper se ejecuta sin abrir DB ni UI.

## Control local autorizado
`gennie-decisions` (`decisions`, activa por defecto) depende de core/db y posee decision_attempts, decision_preferences y runtime externo de Laya. Settings y su cola acotada de guardado pertenecen a decisions; el worker existente persiste y aplica el temporizador tras commit. [D-019](../decisions/laya-settings.md). Bootstrap conserva el módulo, pero Presence no recibe su Controller y ninguna transcripción entra a este router en la app normal. Sus herramientas independientes mantienen sus validaciones y diario. Diario no registra transcripciones ni reproduce efectos inciertos. La intención semántica precede a la disponibilidad; contexto pendiente y comprobado tienen caducidad propia. Adaptaciones opcionales verifican hashes y catálogo; las entrenadas en D-017 siguen experimentales, sin activar. [Diseño vigente](../decisions/semantic-control.md).

## Conversación de Gennie
`gennie-llm` (`llm`, activa por defecto) posee el chat de Gennie. Codex es el proveedor actual, aislado en `codex.rs`; mantiene un proceso hijo app-server, reanuda el `thread_id` de `codex_conversation` y aplica sus preferencias persistidas de modelo, esfuerzo y permisos al crear el hijo. Presence recibe un Controller sin nombres de proveedor y puede pedir otro hilo mediante «Nuevo chat»; el ID activo cambia después de guardarlo. La voz envía cada texto final a este Controller. No abre puertos ni guarda credenciales o audio en SQLite; presence guarda los mensajes finalizados para el historial. Al migrar desde el módulo `codex`, se conserva la selección de activación y el hilo existente. [D-021](../decisions/llm-conversation.md).

## Voz de salida
`gennie-tts` (`tts`, activa por defecto) recibe los textos finales desde presence y los reproduce sin bloquear Slint. Presence solicita su precarga al comenzar el dictado para solapar la carga con la conversación; la carga no produce audio. Posee sus preferencias y un proceso local que libera tras el plazo de inactividad configurado (10 minutos por defecto). No guarda texto ni audio. [D-022](../decisions/tts.md).

## Recordatorios

`gennie-reminders-plugin` (`reminders_plugin`, activo por defecto) expone lectura, escritura y eventos de cambios de Recordatorios mediante EventKit. Bootstrap lo inicia y detiene; ningún consumidor de IA o UI está conectado todavía. No posee tablas ni puertos. [Diseño y verificación](../decisions/reminders-plugin.md).

## Sonda visual Swift Test

`apps/swift-test` es un ejecutable Swift independiente para comparar el vidrio SwiftUI, AppKit y una composición experimental del archivo local del fondo de escritorio en una cara de 128 puntos. No es un `AppModule`, no entra en el workspace Cargo ni en el registro de bootstrap, y no consume SQLite, audio, red ni capturas de pantalla. Presence usa ahora un único material nativo sobre escritorio y apps, mediante `apps/presence/native/face_wallpaper.m`; la composición del archivo de fondo permanece únicamente como variante experimental de la sonda. [D-023](../decisions/wallpaper-face.md) conserva el historial y los límites de verificación.

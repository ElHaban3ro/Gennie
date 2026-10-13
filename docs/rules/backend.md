# Servicios Rust y recursos

Alcance: kernel/core y servicios de apps; composición en bootstrap. Leer [modules](modules.md) al cambiar contratos o lifecycle y [database](database.md) al tocar persistencia.

## Responsabilidades
La app posee sus modelos, repositorio y presentación. `repository.rs` concentra SQL de preferencias; `lib.rs` coordina el módulo y callbacks; `desktop.rs` adapta la política de ventana; `status_menu.rs` posee el icono y menú de barra superior macOS, creados y retirados en main. Separa por responsabilidad cuando crezca; no introduzcas una capa por cada operación pequeña. No hay handlers HTTP, paginación, autenticación ni routers HTTP; el router local de decisiones pertenece a su app. Si aparecen requisitos de red, diseñar y documentar contratos antes de implementarlos; no trasladar automáticamente patrones de servidor al escritorio.

AppState se construye una vez en startup y contiene handles clonables de configuración, DB y Shutdown. No añadir estados de cara, credenciales o repositorios de negocio. Recursos específicos por constructor o en la propia app. Config usa Arc; DB clonada comparte conexión. Kernel es independiente de Slint.

## Errores y tareas
Propaga errores con contexto sin valores sensibles. Logging actual usa campos `event`, `module`, `version`; no publicar URLs, tokens ni contenido de usuario. Un fallo de DB o UI impide declarar arranque correcto. No ocultar fallos persistentes con defaults.

El event loop y objetos Slint viven en main. SQL de escritura interactiva corre en un worker con cola acotada, sin IO de red. No bloquear animaciones con trabajos extensos. La cola debe confirmar commit antes de presentar éxito; errores visibles y logs. Stop desconecta productores, drena y une el worker antes de cerrar DB. No lanzar tareas detached. Para futuros workers largos definir cancelación propia y tiempo máximo; no prometer cierre acotado sin implementarlo.

Fechas SQL: UTC `strftime('%Y-%m-%dT%H:%M:%fZ','now')`. Nombres técnicos en inglés; Optional explícito. No eventos redundantes para interacciones que ya actualizan su propia vista.

## Verificación
Cambios compartidos requieren tests de consumidores y cierre tras errores. No inventar pruebas HTTP en esta app. Ver [testing](testing.md).

## Transcripción local
`apps/transcription` posee captura CPAL, streaming Nemotron y controlador público; presencia solo representa eventos. Audio/inferencia y descarga viven en procesos hijos del mismo binario, supervisados por el padre, sin puertos. Modelo verificado por tamaño/SHA-256, nunca en AppState. Captura acotada, conserva último bloque parcial y drena antes de finalizar. Entrada multicanal conserva el canal de mayor energía con histéresis de 6 dB entre bloques; no promedia voz con canales vacíos ni opuestos. El micrófono se abre después de cargar el modelo y antes de preparar la sesión/stream: su cola guarda el comienzo durante esa preparación. Precarga no abre entrada. Listening se emite al recibir el primer bloque real del micrófono. Resultado vacío produce NoSpeech con diagnóstico separado de entrada ausente (cero muestras), prácticamente silenciosa (pico mono menor de 0.0001) o audio sin palabras reconocidas; este último informa duración recibida. NoSpeech termina la sesión conservando el worker/modelo y su timeout habitual. Contadores y pico son transitorios; no guardar audio ni texto para diagnosticar. Desbordamiento/error visible y liberación del micrófono; nunca descartar audio silenciosamente. Límite 5 min/sesión, watchdog 45 s sin progreso (descarga 900 s), cierre kill+wait de hijos propios y join de lector. Preferencias en worker separado con éxito después del commit. No registrar audio/texto. Ver [decisión](../decisions/transcription.md).

## Decisiones locales
El Controller solicita precarga asíncrona de Laya; ready confirma carga y primer forward, no un efecto. Open_app solo acepta destinos inventariados y refrescados antes del efecto. Abrir apps no consulta reproductores. Ver [diseño vigente](../decisions/installed-apps.md).
La operación precede al enlace de entidades: una pausa no puede convertirse en apertura por coincidir con el nombre Music. Las señales explícitas vetadas impiden efectos aunque el modelo las acepte. Música genérica no fija Apple Music; resolver destino o pedir aclaración. Política y límites en [D-020](../decisions/music-routing.md).
Decisions posee worker serial y helper Python embebido, cancelación por generación, líneas IPC de 16 KiB y espera 120 s. Libera según sus preferencias propias (600 s por defecto o sin plazo), contando desde la última petición terminada; cierre mata grupo de procesos propio y une lector. Solo finales de ASR entran al router; el modelo no suministra código; intención, disponibilidad y ejecución son fases separadas. No cerrar procesos de las apps controladas. Ver [control](../decisions/control.md).

Una apertura se emite una sola vez. Los errores esperables al observar su resultado se recuperan con lecturas acotadas, nunca con otra apertura. Un resultado incierto conserva el helper/modelo y no se convierte en contexto exitoso; el diario distingue failed/uncertain/verified. Errores del helper llevan categorías fijas y etapa de operación; no guardar excepciones, stderr ni argumentos libres del OS. El mensaje al usuario distingue inferencia de ejecución no confirmada.

# Configuración

Propietario: kernel/config. `Config` contiene ruta DB y busy timeout; `Secrets` separado y vacío en esta base, sin Debug ni AppState. Bootstrap consume Secrets al construir clientes futuros. No inventar credenciales. Prefijo GENNIE_. Precedencia: entorno de proceso > `.env` explícito del cwd > defaults. No modificar entorno global mediante unsafe set_var; fusionar valores en memoria. Exportado distinto al archivo genera aviso solo con nombre.

## Variables de proceso
- GENNIE_DATABASE_PATH obligatoria sin default: archivo SQLite, no vacío ni `:memory:`. Instalaciones nuevas usan data/gennie.sqlite3; el .env local conserva la ruta existente al renombrar el producto. Rutas relativas al cwd; para lanzar desde otro lugar proporcionar ruta absoluta y entorno.
- GENNIE_DB_BUSY_TIMEOUT_MS default 500, entero 1..5000. Una conexión, sin parámetros de pool o HTTP.

.env fuera de Git. .env.example lista todas las variables y distingue obligatorias/defaults; test compara declaraciones y defaults. No sobrescribir .env existente. Falta de ruta falla temprano con nombre, no valor. Error de parseo no debe filtrar valores sensibles; cuando se añadan secretos sanitizar mensajes del parser también.

## Preferencias de app
presence posee `reduced_motion` booleano, default false, aplicable sin reinicio, sin unidad. Se carga al abrir; su edición desde la UI está desactivada y el soporte interno queda reservado para configuración futura, con confirmación después del commit. Estado activo/dormido no persiste. No hay app configs ni ConfigSource porque estos ajustes tienen un único consumidor en presence; extraer contrato kernel cuando otro módulo necesite lectura transversal, sin importar repositorio de otra app. Si se añaden catálogos, declarar tipo/default/restricciones/reinicio, rechazar duplicados/defaults inválidos y conservar ajustes de módulos apagados. Aviso futuro configs.changed lleva nombres, sin valores, después de commit. Nunca usar default para ruta externa obligatoria.

La ventana «Apariencia…» del menú Gennie está disponible aunque transcription esté apagado. Presence posee `presence_appearance` (migración 0004, fila única); sus valores enteros son:

| Ajuste | Default | Rango / unidad |
|---|---|---|
| blur_radius | 0 | 0..30 px |
| saturation_percent | 200 | 0..200 % |
| brightness_percent | 101 | 50..150 % (100 neutro) |
| tint_percent | 5 | 0..60 %, relleno grafito global |
| border_percent | 0 | 0..100 %, borde global |
| border_width_hundredths | 67 | 0..300 centésimas de px |
| transition_ms | 300 | 0..300 ms, suavidad al editar el material |

Los sliders previsualizan en vivo sin escribir SQLite. Guardar usa la cola acotada de preferencias y confirma después del commit; no requiere reinicio. Deshacer restaura el último guardado; Valores iniciales recupera los defaults en vista previa. Cerrar la ventana conserva la vista previa durante esa ejecución; sin Guardar se descarta al reiniciar. Copiar valores escribe solo el resumen elegido en el portapapeles por acción del usuario. Desenfoque, saturación, luminosidad, tinte y borde afectan a la misma superficie nativa sobre escritorio y apps. Saturación escala el valor nativo 1,2; luminosidad desplaza sus puntos blanco 0,96 y negro 0,4, sin valores negativos. El radio y el color usan entradas internas del filtro de AppKit, con detección de compatibilidad. La duración controla la transición al editar esos parámetros; `transition_ms` conserva su dato y rango anteriores, pero ya no hay transición por cobertura de ventanas. Si el radio no está disponible, Apariencia informa de la limitación. Los diámetros siguen fijos en 64/128 px. Sin variables GENNIE_ nuevas ni cambios de Config/AppState. El perfil revisado por el usuario el 18 de octubre queda como default del programa: 0/200/101/5/0/67/300. La migración 0006 convierte únicamente la combinación completa de defaults de 0005; conserva cualquier otro perfil guardado.

## Verificación
Validar obligatorias, rangos, precedencia y sincronización ejemplo/declaraciones; reapertura del archivo demuestra persistencia. Ver testing y database para transacciones.

## Preferencias de transcripción
`transcription_preferences` pertenece a transcription: enabled=true; shortcut=Ctrl+Option+Space (parser handy-keys, capturado en UI); mode=hold (hold/toggle); bubble_seconds=15 (2..30); unload_seconds=600 (0..600, cero descarga inmediata; UI en minutos); microphone_name=None (entrada predeterminada del sistema). Entrada explícita identificada por nombre CPAL: ausente o ambiguo produce error, sin fallback silencioso. No guardar índice de enumeración. Migración 0002 añade selección y actualiza el tiempo existente a 600 por petición del usuario, sin cambiar 0001 ni los demás ajustes. Migración 0003 fija el default del globo en 15 s y cambia los valores guardados de 5 a 15; no distingue un 5 elegido explícitamente del antiguo default. Conserva otros tiempos y ajustes guardados. Aplicables después de commit sin reinicio, cancelando dictado en curso al cambiarlas. Modelo Q8_0 fijo, idioma es-ES en esta primera integración. No añadir valores a Config ni secretos. El GGUF reside en `~/Library/Application Support/AIModels/gguf`, compartido por usuario e independiente del checkout.

## Runtime de decisiones
`decision_preferences` pertenece a decisions: preload=true, unload_seconds=600 o NULL para mantener cargado; minutos enteros entre 1 y 120. Migración 0002 conserva 0001 y el diario. Precarga se aplica al siguiente arranque/conexión del Controller; el plazo tras el commit, desde la última petición terminada. Guardado propio para herramientas independientes de decisions; Presence no expone estos ajustes ni crea su Controller en el flujo normal. [D-019](../decisions/laya-settings.md).

`apps/decisions` posee ~/Library/Application Support/soc/decisions/runtime.json: python y model, rutas absolutas existentes, sin defaults externos ni secretos, releídas al crear el hijo. El checkpoint configurado de Laya reside bajo `~/Library/Application Support/AIModels/huggingface/hub`; el entorno Python sigue siendo independiente. No añade variables GENNIE_ ni altera .env. El dictado siempre entrega el texto final a Codex. El runtime de Laya queda disponible para herramientas independientes, sin selección de destino en la UI. Ver [control](../decisions/control.md).

La versión D-017 permite adapter como ruta absoluta opcional de adaptación verificada por hashes. runtime-v2.json tiene prioridad para el binario nuevo; runtime.json se conserva para procesos antiguos. configure_laya --adapter escribe v2 por defecto. Una configuración/adaptación inválida falla; no se oculta con fallback. Ver [semántica](../decisions/semantic-control.md).

## Voz de salida de Gennie
`tts_preferences` pertenece a tts: enabled=true, voice=ef_dora (opciones ef_dora, em_alex, em_santa), speed_percent=100 (70..130) y unload_seconds=600 o NULL para mantener cargado. El plazo admite minutos enteros entre 1 y 120 y cuenta desde la última reproducción terminada. Migración 0002 conserva 0001 y los valores anteriores. Guardado independiente confirmado después del commit y aplicable sin reinicio; Probar voz usa los valores de pantalla sin guardar. El entorno Python sigue en `~/Library/Application Support/soc/tts`; la caché Hugging Face de Kokoro comparte `~/Library/Application Support/AIModels/huggingface` con otros proyectos. Falta de entorno/modelo produce error visible, sin sustitución. No hay secretos ni nuevas variables GENNIE_. [D-022](../decisions/tts.md).

## Conexión con Codex
La sesión ChatGPT pertenece a Codex CLI (`codex login`); gennie no importa tokens en Config, Secrets, AppState ni SQLite. En macOS, preferir el CLI incluido con ChatGPT para acceder al catálogo de modelos vigente y usar el autónomo si falta. `llm_preferences` posee lectura, escritura, comandos y acciones locales booleanos (todos default true), modelo `gpt-6-luna` y esfuerzo `low` por defecto. Escritura requiere lectura; control de apps requiere comandos. Validar nombre de modelo y esfuerzo; comprobar disponibilidad en `model/list` sin sustitución silenciosa. Guardar desde el worker, confirmar tras commit y reconectar el hijo en la siguiente petición. El proceso usa un `CODEX_HOME` aislado con enlace al `auth.json` original cuando existe, sin heredar configuración global, plugins, apps ni MCP. La búsqueda web nativa se habilita siempre en modo live y no ofrece interruptor. Si la sesión global está en otro almacén y no hay archivo, la conexión debe fallar visiblemente. `gennie-llm` guarda solo el `thread_id` en `codex_conversation` para reanudar el mismo hilo. No añade variables GENNIE_ ni altera `.env`. Si la sesión falta o la reanudación falla, mostrar error y conservar el identificador. [D-021](../decisions/llm-conversation.md).

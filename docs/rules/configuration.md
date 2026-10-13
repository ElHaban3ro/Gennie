# Configuración

Propietario: kernel/config. `Config` contiene ruta DB y busy timeout; `Secrets` separado y vacío en esta base, sin Debug ni AppState. Bootstrap consume Secrets al construir clientes futuros. No inventar credenciales. Prefijo SOC_. Precedencia: entorno de proceso > `.env` explícito del cwd > defaults. No modificar entorno global mediante unsafe set_var; fusionar valores en memoria. Exportado distinto al archivo genera aviso solo con nombre.

## Variables de proceso
- SOC_DATABASE_PATH obligatoria sin default: archivo SQLite, no vacío ni `:memory:`. .env local autorizado usa data/soc.sqlite3. Rutas relativas al cwd; para lanzar desde otro lugar proporcionar ruta absoluta y entorno.
- SOC_DB_BUSY_TIMEOUT_MS default 500, entero 1..5000. Una conexión, sin parámetros de pool o HTTP.

.env fuera de Git. .env.example lista todas las variables y distingue obligatorias/defaults; test compara declaraciones y defaults. No sobrescribir .env existente. Falta de ruta falla temprano con nombre, no valor. Error de parseo no debe filtrar valores sensibles; cuando se añadan secretos sanitizar mensajes del parser también.

## Preferencias de app
presence posee `reduced_motion` booleano, default false, aplicable sin reinicio, sin unidad. Se carga al abrir; su edición desde la UI está desactivada y el soporte interno queda reservado para configuración futura, con confirmación después del commit. Estado activo/dormido no persiste. No hay app configs ni ConfigSource porque solo hay un consumidor y un ajuste propio; extraer contrato kernel cuando otro módulo necesite lectura transversal, sin importar repositorio de otra app. Si se añaden catálogos, declarar tipo/default/restricciones/reinicio, rechazar duplicados/defaults inválidos y conservar ajustes de módulos apagados. Aviso futuro configs.changed lleva nombres, sin valores, después de commit. Nunca usar default para ruta externa obligatoria.

## Verificación
Validar obligatorias, rangos, precedencia y sincronización ejemplo/declaraciones; reapertura del archivo demuestra persistencia. Ver testing y database para transacciones.

## Preferencias de transcripción
`transcription_preferences` pertenece a transcription: enabled=true; shortcut=Ctrl+Option+Space (parser handy-keys, capturado en UI); mode=hold (hold/toggle); bubble_seconds=15 (2..30); unload_seconds=600 (0..600, cero descarga inmediata; UI en minutos); microphone_name=None (entrada predeterminada del sistema). Entrada explícita identificada por nombre CPAL: ausente o ambiguo produce error, sin fallback silencioso. No guardar índice de enumeración. Migración 0002 añade selección y actualiza el tiempo existente a 600 por petición del usuario, sin cambiar 0001 ni los demás ajustes. Migración 0003 fija el default del globo en 15 s y cambia los valores guardados de 5 a 15; no distingue un 5 elegido explícitamente del antiguo default. Conserva otros tiempos y ajustes guardados. Aplicables después de commit sin reinicio, cancelando dictado en curso al cambiarlas. Modelo Q8_0 fijo, idioma es-ES en esta primera integración. No añadir valores a Config ni secretos. Almacén macOS en ~/Library/Application Support/soc/models; no ruta relativa al checkout.

## Runtime de decisiones
`decision_preferences` pertenece a decisions: preload=true, unload_seconds=600 o NULL para mantener cargado; minutos enteros entre 1 y 120. Migración 0002 conserva 0001 y el diario. Precarga se aplica al siguiente arranque/conexión del Controller; el plazo tras el commit, desde la última petición terminada. Guardado independiente de Nemotron, fuera de main y confirmado después de persistir. Cargar/liberar manualmente no altera preferencias. [D-019](../decisions/laya-settings.md).

`apps/decisions` posee ~/Library/Application Support/soc/decisions/runtime.json: python y model, rutas absolutas existentes, sin defaults externos ni secretos, releídas al crear el hijo. No añade variables SOC_ ni altera .env. La casilla de control por voz es transitoria y comienza desmarcada por petición del usuario: suspensión temporal de la conexión del dictado al ejecutor, conservando la precarga de Laya y la implementación. Puede reactivarse explícitamente durante la sesión; no promete persistencia. Ver [control](../decisions/control.md).

La versión D-017 permite adapter como ruta absoluta opcional de adaptación verificada por hashes. runtime-v2.json tiene prioridad para el binario nuevo; runtime.json se conserva para procesos antiguos. configure_laya --adapter escribe v2 por defecto. Una configuración/adaptación inválida falla; no se oculta con fallback. Ver [semántica](../decisions/semantic-control.md).

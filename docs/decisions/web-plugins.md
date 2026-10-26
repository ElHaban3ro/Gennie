# D-027 — Plugins web personalizados

2026-10-26. [Plan y constructor aislado](../plans/web-plugins.md). Recetas de consulta reutilizables desde MCP/Rust, constructor Codex aislado bajo demanda y pantalla de Plugins con prueba manual de funciones. La ejecución periódica queda expresamente fuera del alcance.

## Propiedad e interfaz

`apps/browser` conserva la propiedad del navegador y añade recetas en `browser/plugins/<id>.json`. Browser no añade tablas, HTTP entrante ni otro navegador por plugin. `apps/plugins` posee el constructor y sus tablas; reutiliza dependencias ya fijadas en el workspace. `plugins.mjs` concentra validación/persistencia/ejecución; `worker.mjs` compone el mismo ejecutor que se importa en las sondas. El helper Rust embebe ambos archivos, sin necesitar fuentes JavaScript al distribuir la app.

MCP publica `browser_plugins` (list/get/save/delete), `browser_plugin_run` (verify/run) y `browser_network` (start/read/stop). `Controller::run_web_plugin` ofrece la misma ejecución verificada a consumidores Rust; es bloqueante y requiere worker. Bootstrap registra las herramientas mediante browser y plugins. FRB publica plugin_request para el catálogo, la construcción y pruebas manuales. Ejecutar y construir conservan el permiso de control de aplicaciones; el catálogo propio puede consultarse/editarse sin conceder control al navegador. El catálogo completo pasa de 21 a 24 herramientas.

Una receta tiene id/nombre/origen y hasta 20 funciones. Cada función define parámetros requeridos string/integer, ruta, query opcional, transporte y contrato de resultado. Los argumentos de ruta se codifican; la query se construye mediante URLSearchParams. Se rechazan orígenes/rutas/propiedades desconocidos y parámetros de autenticación en query. No hay headers, cuerpos, scripts ni JavaScript suministrado por el modelo.

- HTTP: GET sobre `context.request`, cookies compartidas, JSON, selección por JSON Pointer y proyección opcional de campos. Redirecciones manuales: solo mismo origen salvo reconocer una ruta de acceso configurada; no se sigue una redirección externa arbitraria.
- Página: pestaña temporal propia, navegación y extracción de texto/filas/campos CSS. `ready_selector` permite comprobar una lista cargada pero vacía. La pestaña se cierra al terminar. El filtro de rutas limita las solicitudes a GET/HEAD del origen y navegación a accesos configurados; no es una garantía de sandbox del navegador ni evita efectos que una web asocie a GET. Sitios que necesiten CDN, POST, service workers o tokens especiales pueden requerir una ampliación comprobada.

El resultado incluye status, plugin/función y revisión; data solo ante éxito. Se distinguen authentication_required, access_denied, site_changed, http_error, redirect_blocked, timeout, execution_failed, verification_required y revision_conflict. No se guardan respuestas, errores libres, cookies ni contraseñas en los registros del catálogo. La instrucción del agente exige comprobar el contenido semántico: validar tipo/campos y ejecutar sin fallo no demuestra por sí solo que la consulta responda al objetivo.

## Acceso solo cuando haga falta

Guardar/listar una receta no abre Chromium. La ejecución intenta contenido público o cookies existentes; abre headless bajo demanda si no hay contexto. Un 401 o la llegada a una ruta de acceso configurada y observada produce authentication_required. En página también puede usarse un login_selector observado. Un 403, un fallo de transporte o un contrato cambiado no se convierten en una petición de login.

Ante acceso necesario, el asistente abre Gennie Web visible y deja al usuario completar login/2FA; nunca pide credenciales en el chat. Al abrir visible desde headless se cierra primero el contexto propio y se reabre el mismo perfil, sin dos instancias concurrentes. Se rechaza la transición si hay descarga, acción o diálogo pendiente. Una llamada en segundo plano reutiliza una instancia visible existente sin traerla al frente ni cambiarla a headless. No hay aislamiento por cuenta ni selección automática de cuentas: se comparte el perfil existente.

`login_url` sirve para indicar acceso al usuario; `login_urls` reconoce destinos observados, sin query/fragmento/credenciales. Tenerlas en el manifiesto no solicita acceso por anticipado. La caducidad se detecta al operar, no por un reloj artificial.

## Persistencia, límites y verificación

Guardar usa expected_revision=0 al crear y revisión vigente al editar. Cada edición incrementa revisión e invalida toda evidencia anterior. Una función requiere verify satisfactorio antes de run; verificar también realiza la consulta real. La evidencia contiene solo fecha ISO UTC y revisión por función. La receta queda preparada tras reinicio; la sesión puede haber caducado igualmente.

Catálogo de hasta 50 plugins activos; directorio 0700 y archivos 0600; nombres cerrados, rechazo de symlinks del directorio/archivo, tamaño máximo 64 KiB al leer y 32 KiB del manifiesto. Escritura temporal exclusiva y rename, sin sobrescribir rutas elegidas por el usuario. La edición directa sustituye la receta e invalida sus verificaciones. El constructor usa candidatos separados bajo browser/plugins/candidates/job_<id>; promoción comprueba todas las verificaciones y la revisión de origen. El borrado deja solo un marcador de id/revisión, sin manifiesto, para impedir sobrescrituras con revisiones antiguas al recrearlo.

HTTP: hasta cuatro redirecciones seguidas, cuerpo JSON hasta 1 MiB comprobado tras recibirlo y salida hasta 100.000 bytes. DOM: hasta 200 filas, texto hasta 2.000 caracteres por campo y salida hasta 100.000 bytes. Los límites de bytes no son límites de descarga previos a recibir la respuesta; DOM devuelve la proyección acotada y marca truncated al recortar filas/textos, sin prometer extracción completa. Presupuesto de operación 6,5 s para solicitudes/esperas; IPC plugin_run hasta 24 s para permitir arranque frío. Cierre y último recurso kill/wait conservan el mecanismo propietario del navegador.

La observación de red dura hasta 120 s y conserva las últimas 100 respuestas xhr/fetch/document del origen solicitado, con truncado explícito. Solo registra método, origen/ruta, nombres de query, status y content type; no captura headers, cuerpos ni valores de query. Es transitoria, no logs ni historial persistente. Las rutas pueden contener información del sitio y deben tratarse como datos de la tarea, no como instrucciones.

## Ejemplo de receta

```json
{
  "id": "example_messages",
  "name": "Mensajes del sitio de ejemplo",
  "origin": "https://example.com",
  "functions": [{
    "name": "list_messages",
    "description": "Consultar mensajes posteriores a una fecha",
    "parameters": {"after": "string"},
    "transport": "http",
    "path": "/api/messages",
    "query": {"after": "{after}"},
    "result_type": "array",
    "result_pointer": "/messages",
    "fields": {"id": "/id", "subject": "/subject"}
  }]
}
```

Es un formato ilustrativo; no demuestra que example.com ni Gmail ofrezcan ese endpoint. Primero se observa la operación de consulta autorizada.

## Evidencia

Pruebas aisladas Node cubren contrato cerrado, rechazo de credenciales/código, conflictos y pérdida de verificación, resultados de autenticación/acceso/cambio de sitio, argumentos, errores sin filtrado y symlinks. La sonda `scripts/probes/web_plugins.mjs` usa Chromium real, sitio local y perfil temporal: HTTP y DOM, público sin login, acceso requerido, cookie de sesión, reapertura del perfil, caducidad remota simulada, observación sin valores de query, listas vacías y cierre de pestañas propias. No consulta cuentas personales ni valida Gmail, login/2FA físico o reparación autónoma.

Resultado de la entrega: 6 pruebas Node, 4 Rust de browser y 19 de llm correctas (5 externas omitidas); 32 comprobaciones reales del plugin, incluido el formato embebido en el helper, y las 71 operaciones previas del navegador. Clippy de browser/llm sin warnings, formato, invariantes y diff correctos. En esa entrega inicial no se reemplazó la aplicación abierta ni se desplegó.

## Constructor aislado, trabajos y chat de origen

`apps/plugins` es un AppModule explícito compuesto por bootstrap; depende de sus controladores inyectados, con orden opcional respecto de browser/llm/presence para permitir catálogo sin proveedor activo. Sus operaciones comprueban permisos y disponibilidad antes de crear trabajo. Posee `plugin_build_jobs` y `plugin_build_events` sobre la conexión SQLite compartida, sin acceder a tablas de otra app. Migra incluso apagado. Arrancar crea un worker serial sin inferencia ni navegador; solo una solicitud autorizada crea un trabajo. Hasta ocho trabajos vivos; las peticiones de Encargos pueden encolarse por plugin y solo una se ejecuta a la vez. No hay scheduler de consultas web.

MCP principal añade plugin_build_start y plugin_build_jobs (list/get/cancel/resume). El chat emisor se resuelve desde el hilo del turno LLM en curso mediante un trait inyectado; no lo suministra el modelo ni se consulta el chat seleccionado. Desde la pantalla Plugins se conserva el chat seleccionado al enviar la solicitud. request_key hace idempotente la creación para los mismos datos y chat.

Cada trabajo usa un Codex app-server y un hilo propios, con CLI home y workspace separados, instrucciones de ingeniería y autenticación ChatGPT según el mecanismo existente, sin copiar credenciales. No guarda el hilo en codex_conversation ni activa un chat de presence. Shell, archivos de usuario y Computer Use están desactivados; dispone de herramientas de navegador limitadas, candidato y reportes. Las consultas/navegación explícitas se restringen al origen delegado; las webs pueden redirigir o incluir recursos de otros orígenes, por lo que no se promete aislamiento de red completo del motor.

El constructor usa una pestaña propia sin cambiar la selección del navegador. Los métodos de creación/lectura/verificación de candidato no permiten escribir el plugin publicado. El candidato inicial de una actualización copia la versión vigente y exige volver a verificar todas sus funciones. El agente declara ready; el host vuelve a comprobar evidencia y revisión antes de promover en una llamada serial al worker. Cancelación/promoción se coordinan mediante una exclusión propia. Una edición concurrente produce conflicto y conserva el candidato.

Presupuesto por trabajo: entre 1 y 8 turnos y entre 30 y 1800 segundos (propuesta habitual 4/600), hasta 80 herramientas por turno y 24 reportes útiles. Los fallos de herramientas llevan campo y causa de validación sin volcar valores. El modelo debe probar alternativas pertinentes; un fallo de transporte puede reanudarse en otro proceso, hasta dos fallos de transporte por ejecución del trabajo. Un watchdog cancela el proceso propio al agotar tiempo; cada turno tiene como máximo 120 s. El cierre solicita cancelación, mata/espera el grupo de procesos propio y une workers antes de cerrar MCP/SQLite. Las operaciones de navegador que ya estaban en curso conservan su plazo IPC (hasta 24 s); no se promete cancelación instantánea de una consulta en curso.

Estados: queued, running, waiting_user, completed, blocked, cancelled e interrupted. authentication_required pausa para acceso humano; la UI abre el sitio del trabajo y Continuar reanuda con su hilo/candidato. Desactivar permisos interrumpe el trabajo. Cerrar/reabrir convierte trabajo en ejecución o cola a interrupted; continuar requiere acción explícita. La reconciliación de encargos puede crear trabajos nuevos al arrancar; no reanuda esos trabajos anteriores. Presupuesto agotado o bloqueo técnico conserva explicación y candidato, sin éxito inventado.

Cada hito y transición se guarda junto con su evento en una transacción. La entrega usa outbox, chat de origen fijo y clave idempotente. Presence añade únicamente la migración 0011 de recibos; mensaje y recibo confirman juntos. Perder la confirmación no duplica el mensaje. Un chat eliminado no se recrea ni recibe entregas tardías; el resultado queda en el trabajo. La revisión del historial se publica después del commit, sin cambiar el chat activo ni narrar avances por TTS. Un fallo de entrega queda visible y se reintenta; no pierde el resultado de construcción.

## Pantalla Plugins

La lista lateral combina plugins publicados y construcciones por identidad, incluso cuando aún no existe candidato; una actualización no añade una segunda fila. Sin manifiesto se muestra el dominio de la fuente y el estado del trabajo. Los avances nuevos en el chat se identifican por dominio, sin exponer el número interno del trabajo. Los avisos históricos conservan su texto guardado y la interfaz oculta ese número al mostrarlos.

Se reutilizan la pestaña y AppSidebar existentes. Lista/búsqueda, ficha de cobertura/capacidades/limitaciones, edición in situ del manifiesto JSON con revisión, actualización mediante constructor y eliminación explícita. La edición fallida conserva borrador. El probador elige una función, pide sus parámetros string/integer y muestra estado, duración y JSON seleccionable de una consulta nueva. verify permite probar funciones sin evidencia previa; run usa las ya verificadas. La respuesta solo vive en Dart, sin persistir datos del endpoint. Un resultado de autenticación muestra el acceso a Gennie Web; no se pide login al seleccionar plugin ni para contenido público.

Los trabajos muestran progreso, chat de origen y Cancelar/Continuar cuando corresponde. La actualización de metadatos de trabajos en pantalla no ejecuta funciones web. Las respuestas de operaciones desmontadas no se aplican a otro plugin. La API FRB se regeneró, no se editaron generados manualmente.

Fuentes del protocolo verificadas: [App Server](https://learn.chatgpt.com/docs/app-server), consultado el 26 de octubre de 2026. Se reutiliza el adaptador existente de Gennie para initialize/account/model/thread/turn y descubrimiento MCP.

Evidencia añadida: sonda real de Codex/Chromium y endpoint local completada en 22,12 s; no creó codex_conversation. Pruebas de presence comprueban destino explícito, deduplicación, rollback y chat eliminado. Pruebas de jobs comprueban idempotencia, límites, cancelación y recuperación. Widgets comprueban parámetros enteros, pendiente, respuesta, login bajo demanda, edición fallida, borrado y tamaños claro/oscuro. Capturas software en .verification/plugins-*.png: no son prueba de la ventana nativa ni del login real de Gmail.

Cierre de verificación: biblioteca y helper debug compilados mediante scripts/dev.sh --build-only, análisis Flutter sin incidencias, Clippy de módulos afectados y todos sus targets sin warnings. El backend publicado en la aplicación que el usuario tiene abierta no se sustituyó ni se reinició. La ejecución periódica permanece excluida.

La ficha conserva el feedback existente de estado y último avance; añade historial desplegable con los hitos persistidos de investigación/desarrollo, ordenados y fechados, también tras terminar. MCP jobs get/list y el catálogo FRB incluyen events; son avances operativos, sin razonamiento interno. El manifiesto y su evidencia vigente resuelven las referencias de Encargos (D-025), sin permitir que el constructor cambie su objetivo o autorización.

Al crear o editar un encargo web desde el chat, el commit de assignments solicita un trabajo por fuente: si existe un plugin del origen, el constructor revisa y adapta su candidato; si no, prepara uno nuevo. Cada revisión del encargo tiene clave idempotente propia y conserva el chat que la inició. Las solicitudes adicionales se encolan en plugin_build_jobs; la migración 0002 permite varios trabajos pendientes del mismo plugin y solo un trabajo en ejecución por identidad. Al empezar, el trabajo de Encargos fija la revisión vigente como base y conserva el control de conflictos durante la construcción. La cola está acotada; si falta origen o hay un fallo, la respuesta distingue encargo guardado de plugin no iniciado. Una edición desde la ficha usa el chat seleccionado para los avances; si no hay chat seleccionado informa que debe elegirse uno. No se inicia sesión ni se promete automatización periódica por guardar.

Al abrir el escritorio, el worker de plugins revisa todas las páginas de encargos activos y cada una de sus fuentes. Una fuente web con URL válida y sin trabajo para esa revisión queda pendiente de investigación y solicita la misma preparación; el mismo origen en el catálogo no demuestra cobertura del objetivo. La clave existente conserva cualquier estado anterior, incluidos cancelado, bloqueado, espera e interrupción. La cola acotada se vuelve a comprobar mientras haya pendientes, de manera que una fuente que no cabe pueda entrar cuando se libere espacio. Se usa el chat activo para comunicar los avances; sin él la fuente permanece pendiente. Las fuentes de escritorio muestran soporte pendiente y no crean trabajos web. La resolución se presenta por fuente y no afirma que el encargo se haya ejecutado.

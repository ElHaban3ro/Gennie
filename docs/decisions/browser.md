# Navegador persistente de Gennie — D-026

## Requisito y alcance

Un navegador dedicado que pueda abrir el usuario desde la cabecera del chat para iniciar sesión y preparar sitios. Playwright y Gennie comparten esa misma instancia y perfil; no se copia ni automatiza el navegador personal. No se incorporan conectores de IA ni un servidor HTTP/CDP.

`apps/browser` posee Controller, proceso Node y contexto Chromium. Bootstrap inyecta el mismo controlador en el runtime, las herramientas MCP y el módulo de lifecycle. El frontend solo llama `open_browser` por FRB. Arrancar el módulo no abre ventanas, instala paquetes ni accede al perfil. El navegador se abre únicamente por acción explícita; repetir apertura trae al frente la pestaña seleccionada. Cerrar manualmente Chromium permite abrirlo de nuevo con el mismo perfil. Dos instancias de Gennie no pueden compartirlo simultáneamente: Chromium conserva su exclusión y el error se presenta sin cambiar a un perfil temporal.

## Datos locales

### Identidad Gennie Web

El nombre visible del navegador en macOS es **Gennie Web** (`com.gennie.web`), con un globo de trazo blanco sobre el mismo fondo oscuro de Gennie. SVG fuente y PNG/ICNS en `apps/browser/assets`; el botón del chat comparte el motivo de globo y usa «Abrir Gennie Web».

`scripts/brand_browser.py` prepara una copia privada versionada en `runtime-1.64.1/Gennie Web.app`, partiendo del Chromium instalado por Playwright. Usa clonación APFS cuando está disponible, modifica solo identidad/icono del paquete y nombres localizados, conserva copyright/licencias y firma local ad hoc. No toca el navegador descargado original ni el perfil. La firma se verifica antes de colocar el bundle; una marca de versión/icono permite reutilizarlo. Actualizar un bundle abierto requiere cerrar solo ese navegador. Playwright recibe explícitamente su ejecutable, sin fallback silencioso al navegador original. Los nombres de binarios, framework y textos internos de Chromium permanecen; no es una compilación propia del motor ni una distribución notarizada.

La preparación estándar con `scripts/setup_browser.sh` incluye este paso. Requiere Python 3 y herramientas macOS de copia/firma, sin paquetes Python adicionales. Los iconos raster se generan desde el SVG con `rsvg-convert` e `iconutil` al modificar el diseño; los artefactos finales se versionan y no requieren ese renderer durante instalación.

Verificado el 26 de octubre: `NSRunningApplication` devuelve nombre Gennie Web y bundle `com.gennie.web` para el navegador real, y su icono del sistema corresponde al globo. La sonda de cookies/localStorage pasó usando el bundle personalizado; preparación repetida valida y reutiliza el paquete firmado. Compilación release/firma y prueba del botón correctas. Se reabrió únicamente el navegador conservando la ruta del perfil; no se reinició Gennie.

- Perfil fijo: `~/Library/Application Support/Gennie/browser/profile`.
- Runtime versionado: `browser/runtime-1.64.1`, Playwright y playwright-core exactos, npm lock con integridad. `node` enlaza al ejecutable local preparado; no depende del PATH del runner. Node debe seguir instalado en esa ubicación.
- Navegadores descargables: `browser/browsers`, separados del perfil y de los datos de SQLite. Directorios privados 0700.

El contexto usa `launchPersistentContext`, modo visible por apertura manual o headless bajo demanda por plugins ([D-027](web-plugins.md)), tamaño nativo y restauración de última sesión. Activa el sandbox de Chromium y conserva la advertencia self-XSS de DevTools. Mantiene el almacén de credenciales predeterminado de Playwright; no debe usarse como gestor de contraseñas. Conserva cookies y almacenamiento de sitios según Chromium; no promete que una cuenta permanezca autenticada indefinidamente. Caducidad, cierre de sesión remoto y políticas del sitio siguen aplicándose. `sessionStorage` y otras estructuras efímeras conservan su semántica de navegador; no se exportan cookies, contraseñas ni tokens al historial o logs. No se sincroniza con otro dispositivo ni existe un borrado automático del perfil.

## Control y cierre

### Preferencia por la API y proactividad

Gennie recibe en developerInstructions un inventario calculado con los nombres de herramientas realmente registrados y los permisos vigentes: navegador por API disponible, herramientas concretas, control del ordenador permitido y red de comandos desactivada. Se incluye al crear, reanudar y crear otro chat; el descubrimiento MCP completo sigue siendo requisito de conexión. No habilita permisos nuevos ni reemplaza el texto del usuario. La evidencia actual prevalece sobre negaciones antiguas del historial.

Si se pide abrir algo en Gennie Web, la primera vía es MCP/Playwright. Para investigación general puede empezar por búsqueda web o lectura directa; ante acceso denegado o contenido insuficiente, intenta el navegador propio sin esperar otra orden del usuario. Secuencia: consultar pestañas, abrir si hace falta, navegar y leer snapshot. La navegación informa además del status HTTP. Si esa vía falla por una causa comprobada y aún hay una alternativa pertinente, puede usar Safari u otro navegador mediante las herramientas de ordenador existentes y permisos activos. No equipara abrir la URL con leerla, no inventa Computer Use instalado y no elude un permiso desactivado. Las lecturas dentro del objetivo aceptado no necesitan confirmaciones repetidas; envíos/cambios externos conservan el alcance autorizado. Solicitar texto/captura queda después de agotar las alternativas pertinentes y permitidas.

Diagnóstico del 26 de octubre: la instancia abierta ya publicaba seis herramientas browser_* y respondió a browser_snapshot con open=true, sin control del ordenador. La sonda optativa de decisión real falló con las instrucciones anteriores (cero llamadas al navegador y abandono tras búsqueda) y pasó con la política/inventario nuevos: snapshot → open → navigate → snapshot, devolviendo el dato observado en una página simulada. Es una muestra real de elección de herramientas, sin acceso a cuentas ni prueba de Safari; no garantiza todas las decisiones del modelo. Los cambios requieren cargar la biblioteca recompilada y una sesión con las instrucciones vigentes; no se reinició la instancia del usuario.

Fuentes del protocolo: [MCP en Codex](https://learn.chatgpt.com/docs/extend/mcp) y [App Server](https://learn.chatgpt.com/docs/app-server), consultadas el 26 de octubre de 2026. El registro de herramientas, las instrucciones de servidor y developerInstructions orientan al modelo; no ejecutan automáticamente una navegación por sí solos.

### Catálogo ampliado (24 herramientas)

Todas siguen dentro del MCP local `gennie`, junto con `assignments_*`. No se añadió un servidor Playwright independiente ni un puerto de producto.

| Herramientas | Función |
|---|---|
| browser_open, browser_snapshot, browser_navigate | Ventana compartida, pestañas/frames/árbol accesible y navegación con status HTTP |
| browser_tabs | Listar, crear, seleccionar y cerrar pestañas observadas |
| browser_history | Atrás, adelante y recarga |
| browser_find, browser_read | Buscar texto y extraer texto/artículo/enlaces/tablas de un elemento/frame |
| browser_wait_for | Esperar elemento o texto visible/oculto, URL o carga; hasta 8 s |
| browser_click, browser_fill, browser_press | Interacción con elementos observados |
| browser_hover, browser_scroll | Desplegar contenido y desplazarse por API |
| browser_select_option, browser_check | Selects por valor/etiqueta y casillas/radios |
| browser_dialog | Consultar, aceptar y descartar alert/confirm/prompt pendientes |
| browser_screenshot | JPEG de viewport, página completa o elemento; imagen MCP y archivo local |
| browser_request | HTTP GET/HEAD con cookie jar del contexto; sin extracción de cookies ni headers sensibles |
| browser_download, browser_upload | Gestionar descargas persistidas y adjuntar archivos locales elegidos para la tarea |
| browser_pdf | Exportar la página a PDF sin diálogo de impresión del sistema |
| browser_network | Descubrimiento temporal de metadatos de red por origen |
| browser_plugins, browser_plugin_run | Recetas web versionadas y ejecución/verificación HTTP/DOM; ver D-027 |

Snapshot identifica hasta 200 elementos visibles de hasta 16 frames y devuelve referencias asociadas a handles concretos; conserva selectores como alternativa explícita. Cada snapshot/find y cada acción invalida las referencias anteriores. Antes de usar una referencia se comprueba conexión e identidad del elemento; ante referencia caducada se exige observar de nuevo. No hay JavaScript arbitrario suministrado por el modelo. Find también localiza texto que no estaba entre los primeros targets del snapshot. La lectura y las respuestas se acotan para no volcar páginas ilimitadas al modelo.

Las acciones devuelven `needs_snapshot`. Si un diálogo interrumpe una acción, se conserva su promesa y se devuelve `modal`/`action_pending`; aceptar o descartar drena la acción anterior y comunica incertidumbre si no se confirmó. No se repite el click que abrió el diálogo. El selector de archivos se resuelve con `browser_upload`, incluido `files=[]` para cancelarlo.

### Archivos, sesión y permisos

Capturas, PDF y descargas se guardan en `~/Library/Application Support/Gennie/browser/artifacts`, con directorio 0700 y archivos 0600, nombres generados sin sobrescribir archivos elegidos por el usuario. Son resultados persistentes; no se borran al cerrar el contexto. Las descargas se copian a un archivo parcial propio y se renombran al completar; se retienen/observan sus tareas y se cancelan las pendientes al cerrar. `download wait` admite esperar la primera descarga sin conocer aún su ID y consulta explícita por ID en las siguientes.

Upload acepta hasta 8 archivos regulares de 25 MiB cada uno, con rutas absolutas resueltas. Adjuntar archivos requiere el permiso vigente de lectura; cancelar/vaciar una selección no requiere leer archivos. El asistente solo adjunta archivos elegidos por el usuario para esa tarea, pues cambiar el campo puede iniciar una carga del sitio. Las demás herramientas requieren control de apps/comandos; el botón de apertura manual sigue siendo independiente. No se amplían permisos.

`browser_request` admite GET/HEAD, usa `context.request`, sigue hasta cinco redirecciones, entrega status/tipo/cuerpo de texto acotado y libera cada respuesta. Comparte cookies con el contexto, pero no incorpora automáticamente tokens de localStorage ni garantiza acceso a APIs privadas. No devuelve headers de autenticación o Set-Cookie. Para otros tipos binarios se informa del tipo sin volcar el cuerpo. La respuesta textual se limita a 20.000 caracteres; recursos de más de 8 MiB se rechazan tras recibirlos. La captura JPEG se limita a 2 MiB y el PDF a 32 MiB; no se promete un límite de descarga de red previo a recibir esos bytes.

### Transporte, plazos y cierre

IPC JSON por stdin/stdout: peticiones de 128 KiB y respuestas de hasta 4 MiB. El transporte MCP común admite contenido `image` además del JSON de negocio, sin duplicar el base64 en structuredContent. El sobre interno `__gennie_mcp_output_v1` queda reservado a resultados multimodales construidos por herramientas propias. Encargos mantiene su contrato JSON. Los sockets aceptados se fijan explícitamente en modo bloqueante con timeout: en macOS podían heredar O_NONBLOCK y truncar una imagen a los primeros 8 KiB. La prueba del helper stdio conserva una imagen de 200.000 caracteres.

Llamadas serializadas con rechazo ocupado y sin reintentos automáticos. Plazos Playwright: acciones 6 s, navegación 8 s, arranque 9 s; espera IPC 12 s (24 s para plugin_run con arranque frío). Esperas/descargas acotadas a 8 s por petición. Snapshot tiene presupuesto de exploración y conserva truncado explícito; no equivale a leer toda página. Timeout comunica resultado incierto, sin registrar excepciones/URLs. Cierre: EOF, cancelación de descargas pendientes, `context.close()` y drenaje de tareas propias, con hasta 5 s para terminar; último recurso kill/wait del grupo propio y error de escritura final no confirmada. Los hijos de Chromium los administra Playwright y el lector se une antes de finalizar. El perfil y los artifacts completados se conservan; el cierre global ocurre antes de SQLite.

Fuentes públicas verificadas: [Locator](https://playwright.dev/docs/api/class-locator), [APIRequestContext](https://playwright.dev/docs/api/class-apirequestcontext), [Download](https://playwright.dev/docs/api/class-download) y [diálogos](https://playwright.dev/docs/dialogs). Reutiliza Playwright 1.64.1 y Node instalados, sin dependencias de registro nuevas.


## Preparación y uso

Al iniciar una sesión del navegador se muestra `https://gennie.io/`. Reutiliza su pestaña si se restauró, sustituye una pestaña vacía o crea una nueva conservando las demás. Pulsar Abrir con el navegador ya abierto solo lo trae al frente; no interrumpe la navegación actual. Cookies y perfil permanecen. Si la página no carga, se informa que el navegador está abierto y se puede recargar manualmente; no se reinicia ni se cambia de perfil.

Ejecutar `./scripts/setup_browser.sh` con Node >=20 instalado. Usa `npm ci --ignore-scripts` e instala Chromium desde Playwright; no inicia el navegador ni altera el perfil. La instalación es explícita y externa al build: tanto desarrollo como release usan el runtime local del usuario; empaquetar una distribución autónoma de Node/Chromium queda fuera de este cambio.

`node scripts/open_browser.mjs` permite abrir el navegador dedicado sin reiniciar Gennie. Ese proceso termina al cerrar su navegador. Debe cerrarse antes de que otra instancia tome el perfil; no hay control cruzado entre procesos independientes. En la app, usar «Abrir Gennie Web», a la izquierda del menú de tres puntos. Mientras abre, el botón muestra progreso y bloquea pulsaciones repetidas; errores mediante el flujo común del escritorio.

## Verificación

`node scripts/probes/browser_persistence.mjs` crea un perfil temporal y comprueba cookies persistentes/localStorage tras cerrar, abrir desde el helper real y reabrir Chromium. Registra por separado si Chromium restaura la cookie de sesión. Comprueba apertura repetida, rechazo de `file:` y salida limpia del helper. No usa cuentas personales ni demuestra autenticación en un sitio concreto.

Tests Rust: módulo desactivado, petición concurrente, runtime ausente y validación MCP sin crear procesos. Widgets: ubicación junto al menú a 640 × 420, progreso/bloqueo, recuperación tras error y aviso visible. Compilación/análisis no sustituyen una comprobación física del botón en una instancia recompilada.

Verificación inicial del 26 de octubre, antes de ampliar el catálogo: sonda real correcta con sandbox activado (incluida cookie de sesión), 20 tests Rust de browser/bootstrap y 21 widgets de escritorio correctos, análisis Dart sin incidencias, Clippy de browser/bootstrap con warnings denegados, invariantes y formato correctos. Paquete macOS release compilado y firma local verificada. Navegador dedicado abierto mediante el lanzador manual y perfil fijo; instancia del escritorio no reiniciada por petición del usuario. Pendientes la pulsación real del botón en la nueva versión, descubrimiento de estas herramientas en una sesión Codex real y autenticación en sitios concretos.

Fuentes: [contexto persistente](https://playwright.dev/docs/api/class-browsertype#browser-type-launch-persistent-context), [perfiles y estado](https://playwright.dev/mcp/configuration/user-profile). Dependencias y límites en [dependencies](dependencies.md).

## Evidencia de la ampliación — 26 de octubre de 2026

La sonda real de capacidades pasó 71 operaciones con Chromium headless, un sitio local temporal y perfil independiente: incluyó frames/referencias, formularios, esperas, pestañas/historial, diálogos, subida/vaciado, descarga completa y cancelación en curso, capturas de viewport/página completa/elemento, PDF y HTTP con cookies compartidas/status 403. La sonda independiente por stdio conservó cookies/localStorage tras cerrar y reabrir el helper. El app-server Codex instalado descubrió las 21 herramientas con CODEX_HOME vacío y sin inferencia. El test del puente stdio preservó 200.000 caracteres de imagen y el JSON de datos por separado.

El workspace Rust, Clippy, formato y comprobación del diff pasaron. `./scripts/build.sh` produjo el paquete macOS release con firma local ad hoc; no se instaló ni abrió. Estas pruebas no usan cuentas personales ni prueban decisiones del modelo con todo el catálogo, sesiones en sitios privados o interacción física en Safari. No se reinició la instancia del usuario; para cargar el catálogo ampliado y el helper nuevo hace falta arrancar la versión recompilada.

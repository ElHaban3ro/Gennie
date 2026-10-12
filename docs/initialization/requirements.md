# Requisitos vigentes — primera base soc

Estado: primera base implementada y verificada localmente, checkout actual main sin commits, origin git@github.com:ElHaban3ro/soc.git. Repositorio remoto vacío según usuario; no accedido ni verificado. Pendientes históricos de carpeta/Git/macOS/activación manual están resueltos.

Producto futuro: asistente personal proactivo integrado al OS para productividad/decisiones. Viabilidad de reutilizar cuotas ChatGPT/Grok no verificada; fuera de esta base. Autonomía y tareas reales se definirán después.

Base confirmada: macOS primero, orientación multiplataforma; Rust modular, kernel y SQLite junto a interfaz nativa sin WebView; sin HTTP/usuarios/sesiones/grandes dominios. App dedicada de presencia. Cara siempre visible: reposo pequeño dormido con zetas, activo grande feliz; clic alterna; siempre encima. Expansión suave con rebote ligero y contracción tranquila aceptadas. Cierre claro; no dejar procesos temporales.

Elecciones delegadas: Slint 1.18.1, módulos y recursos concretos, preferencia reduced_motion persistida, tonos monocromáticos/tamaños/esquina inferiores revisables; no son requisitos nuevos del usuario.

No commit/push/despliegue, worktree ni cambio de rama. No tocar otros proyectos. Documentos consolidados son fuente vigente; historial completo de respuestas a continuación, sin pérdida de decisiones previas.

## Historial de respuestas conservado

# Respuestas y requisitos

Estado: inicialización completada; archivo histórico. Fuente vigente en docs; cierre y límites al final.

## Respuestas del usuario
- Directorio indicado: /Users/fer/Documents/Projects/Fer/Dev.
- Solicitud: definir e iniciar una aplicación Rust siguiendo el prompt íntegro conservado en initialization-prompt.md.

## Interpretaciones
- Ningún producto ni módulo de dominio definido todavía.
- Pendiente aclarar si Dev será la raíz final o la carpeta contenedora de una subcarpeta del proyecto.

## Preguntas pendientes
- ¿Cuál será el nombre técnico o carpeta final? Nombre de la aplicación confirmado: soc.
- ¿Qué hace, qué necesidad resuelve y quién la utilizará?
- Alcance inicial, datos, interfaz, entorno, integraciones y criterios de entrega.
- Git: proyecto nuevo o existente, URL sin credenciales, checkout local, rama y remoto; autorizaciones por definir.

## Requisitos confirmados
- Conservar el prompt completo y registrar respuestas, decisiones y avance antes de generar la base.

## Respuesta registrada — 2026-10-11
- Usuario: la aplicación se llamará «soc».
- Pendiente: función, necesidad, público y carpeta final.

## Propósito descrito por el usuario — 2026-10-11
- Crear un asistente virtual personal propio, proactivo, integrado directamente con el sistema operativo.
- Mejorar la productividad y la velocidad de decisión; realizar tareas y tomar decisiones por el usuario.
- Reutilizar las cuotas de asistentes que el usuario ya paga, por ejemplo ChatGPT y Grok.
- Las referencias de productos mencionadas por voz son ambiguas; no se han identificado ni verificado.

## Interpretación pendiente de concretar
- El objetivo sugiere coordinación de tareas y acciones del sistema, pero no determina todavía herramientas, interfaz ni arquitectura específica.
- La proactividad deseada no autoriza acciones concretas ni define todavía qué decisiones serán automáticas o requerirán aprobación.
- Reutilizar suscripciones es un requisito deseado, no una integración cuya viabilidad haya sido confirmada. Verificar las interfaces admitidas y condiciones de cada proveedor antes de decidir conectores; no asumir equivalencia entre suscripción y acceso API.

## Próximos datos necesarios
- Sistema operativo de la primera versión.
- Primeras tareas concretas que deberá ejecutar.
- Límites de autonomía y aprobación.

## Plataformas — respuesta del usuario, 2026-10-11
- Primera versión y primer hito: macOS, donde el usuario desarrolla.
- El producto no será exclusivo de macOS; se orientará a funcionar en otros sistemas operativos.
- Pendiente definir qué otros sistemas tendrán soporte, su prioridad y criterios de verificación.
- Pendiente definir las primeras tareas concretas y límites de autonomía.

## Alcance inicial — respuesta del usuario, 2026-10-11
- No desarrollar grandes funcionalidades de dominio en esta primera versión; construir juntos de forma gradual.
- Ejecución local.
- Persistencia SQLite.
- Definir bien las bases del kernel, incluida la comunicación con base de datos.
- No incluir gestor de usuarios ni sesiones en el alcance inicial.
- Prioridad: establecer, crear y definir la parte gráfica para evaluar una base visual y recibir feedback agradable antes de avanzar.
- La pregunta anterior sobre dos o tres tareas se reemplaza por la definición de la base; no exigir funcionalidades de dominio para empezar.

## Pendientes estructurales actuales
- Tipo de interfaz: escritorio nativo sin WebView o escritorio con tecnologías web; stack todavía no elegido.
- Forma visual inicial, referencias y criterio de aceptación de la base gráfica.
- Nombre técnico/carpeta final; soc es el nombre confirmado y Dev el directorio indicado.
- Estado de Git y checkout.
- Modelo mínimo de datos que necesita la base; no inventar entidades de dominio.

## Interfaz nativa y forma visual — respuesta del usuario, 2026-10-11
- Interfaz de escritorio nativa, sin WebView.
- Slint es una opción sugerida por el usuario, todavía no decidida; pide valorar su adecuación.
- Estado de reposo: pequeño círculo con una carita en una esquina de la pantalla.
- Al llamar al asistente, hablar o estar activo/en primer plano, el círculo crece y se transforma/muestra una cara; al terminar vuelve a achicarse.
- No se desea la presentación de una ventana convencional de aplicación.
- El ejemplo de revisar correos ilustra el comportamiento futuro; esa integración y la activación por voz no forman parte de la primera versión.
- Pendiente concretar cómo activar y demostrar los estados visuales en la base inicial.

## Interpretación técnica por validar
- La forma flotante requerirá evaluar ventana sin marco, transparencia, posición, política de foco y comportamiento sobre otras ventanas. La necesidad de estar siempre encima todavía no está confirmada.
- No interpretar nativo como exigencia de controles del sistema: Slint es candidato a interfaz dibujada sin WebView.

## Estados visuales y módulo — respuesta del usuario, 2026-10-11
- Acepta como comportamiento base la demostración por clic propuesta, sin voz ni correo.
- La bolita debe permanecer siempre por encima de otras aplicaciones.
- Corrección: la cara debe estar visible tanto en reposo como en actividad; no aparece únicamente al expandirse.
- Reposo: bolita pequeña, cara dormida y animación de zetas. «Apagada» describe el reposo visual, no la terminación del proceso.
- Activo: bolita más grande y cara feliz/contenta.
- Prefiere una app dedicada a esta interfaz; delega el encaje concreto conforme a los límites del prompt.

## Git — respuesta del usuario, 2026-10-11
- El repositorio Git todavía no existe.
- El usuario lo creará y compartirá su dirección después.
- URL, rama, remoto y checkout: pendientes.
- No se solicita crear el remoto ni inicializar Git en nombre del usuario.

## Repositorio recibido — 2026-10-11
- URL SSH sin credenciales: git@github.com:ElHaban3ro/soc.git.
- Estado remoto (vacío o con contenido): no verificado.
- Checkout existente, ruta local, rama y remoto local: pendientes.
- Compartir la URL no autoriza por sí solo clonado, commit, push ni publicación.
- Propuesta pendiente: usar /Users/fer/Documents/Projects/Fer/Dev/soc como carpeta final si no existe checkout previo.

## Estado vigente del checkout — 2026-10-11
- Usuario autorizó inicializar Git dentro de la carpeta; confirmó que el remoto está vacío (no verificado mediante acceso remoto).
- Carpeta final: /Users/fer/Documents/Projects/Fer/Dev/soc.
- Repositorio local inicializado, sin commits.
- Rama inicial elegida por la configuración de Git: main; no se cambió de rama.
- Remoto local origin: git@github.com:ElHaban3ro/soc.git.
- Documentación trasladada de Dev/.creation a soc/.creation; contenido verificado byte por byte antes de actualizar registros.
- Sin commit ni push; no se contactó el remoto.

## Corrección del orden y animaciones — 2026-10-11
- Usuario pide incorporar kernel y SQLite desde la primera base, independientemente de la interfaz elegida; no posponerlos a después de una demo gráfica aislada.
- Antes de construir, evaluar que la interfaz permita añadir animaciones: expansión suave del círculo y posible rebote o efecto similar a físicas.
- Rebote y físicas son posibilidades exploratorias, no una exigencia de motor físico.

## Autorización de construcción — 2026-10-11
- Usuario acepta la propuesta de rebote ligero al expandirse y contracción más tranquila si se siente mejor.
- Usuario ordena comenzar la construcción de la base conjunta: kernel, SQLite e interfaz nativa.
- Sin autorización de commit, push o publicación.

## Corrección visual explícita — 2026-10-11
- Usuario: «la cara debe ser monocromática, moderna y minimalista». Reemplaza el supuesto de azul y mejillas rosadas.
- Permanecen tamaño dormido/activo, cara visible, zetas, animación y superficie nativa.

## Cierre de la primera base — 2026-10-11
Estado vigente: inicialización completada, con límites de validación runtime documentados. Fuentes vigentes: docs/initialization/requirements.md, docs/initialization/implementation.md y docs/decisions/README.md. Este archivo conserva origen e historial, no es una segunda fuente vigente.
- Base construida: kernel/config, kernel/db, kernel/core, apps/presence y bootstrap. SQLite compartido, migraciones atómicas con SHA-256 por módulo, registro/DAG y cierre inverso.
- Cara monocromática minimalista conforme a D-013. Clic/Espacio, zetas, spring/contracción, M persistido, A y menú Acerca, Escape/clic secundario/menú Salir.
- Verificación final: fmt, 13 tests, Clippy con -D warnings, build y check_invariants verdes. UI observada en macOS; M confirmado por lectura SQLite y reapertura sin migraciones repetidas.
- Límites: capturas secuenciales no equivalen a revisión continua del movimiento. Foco de otras apps, transparencia compositada, always-on-top y paso real de clics requieren comprobación manual OS; automatización app dirigida no demuestra esas garantías. Multi-monitor/Retina/Spaces/otros OS no probados. Click-through específico macOS, no implementado en otros OS.
- Prompt SHA-256 original conservado: 7412501ba941c357418096079a62cff4487dccf9cb138f197c36f4088ea3d9dd. Sin commit/push, rama main y origin conservados, sin procesos temporales vivos.
Último paso completado: construir/verificar/consolidar. Siguiente: evaluación manual de comportamiento de ventana y feedback del usuario; no hay bloqueo de configuración ni de compilación. Conectores y autonomía quedan fuera de este hito.

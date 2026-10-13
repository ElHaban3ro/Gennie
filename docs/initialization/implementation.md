# Primera base implementada — 2026-10-11

Estado: construcción finalizada. No hay bloqueo de configuración/compilación. Falta evaluación manual específica de comportamiento de ventana descrita abajo; no se declara validación completa del OS.

## Construido
Workspace de cinco crates útiles, sin capas vacías: configuración explícita Config/Secrets; SQLite único compartido con foreign keys/WAL/FULL y busy timeout; migraciones por módulo atómicas, versionadas y protegidas por SHA-256; registro explícito con DAG/selección conservada; arranque y cleanup inverso; worker de preferencias confirmado después de commit y error visible.

Presence posee UI nativa Slint 1.18.1/Winit/FemtoVG sin WebView: monocromática moderna/minimalista, dormida pequeña con zetas y activa feliz expandida, cara siempre visible, expansión/contracción y gestos graduales, respiración sutil y zetas con ritmos independientes, movimiento reducido persistido. Mac Accessory sin activación forzada, no-frame/transparent/always-on-top, ubicación diferida hasta existir ventana y hit test circular dinámico. Icono/menú de barra superior macOS con Asistente → Despertar / Descansar, Acerca de soc y Salir de soc. Menú nativo de información y cierre; atajos y ayuda sobre la cara desactivados, con soporte de preferencias reservado para configuración futura.

## Sueño y despertar suaves — 2026-10-11

La cara combina respiración de amplitud variable y desplazamiento interno lento, con períodos independientes para evitar una repetición corta evidente. Ojos y boca interpolan sus curvas al alternar; el despertar levanta ligeramente los rasgos y forma la sonrisa gradualmente. Las zetas aparecen/desaparecen sin saltos visibles y se desvanecen durante el despertar. El reloj se conserva al alternar y movimiento reducido deja la cara estática. El diámetro efectivo alimenta el hit test nativo existente; no se cambia la posición de ventana ni su interacción.

Verificación de este cambio: compilador Slint 1.18.1 ejecutado con el build script existente y salida temporal, sin errores; comprobación de invariantes correcta. Build locked del workspace bloqueado por desajuste de dependencias/lockfile en el trabajo simultáneo de dictado. Fmt bloqueado por apps/transcription/src/controller.rs ausente durante esa implementación. No se alteran esos archivos ni el lockfile para resolver un cambio ajeno. No se afirma revisión visual continua ni ejecución de estas animaciones en la instancia existente del usuario; requieren recompilar y evaluar cuando el workspace esté consistente.

## Idle despierto — 2026-10-11

La cara activa también tiene movimiento ambiental sin interacción: respiración de hasta 0.35 px en diámetro, desplazamiento lento de mirada/rasgos de hasta 2.2 px, variación mínima de sonrisa y parpadeos suaves de unos 320/360 ms. Dos ritmos de parpadeo con pausas moduladas evitan una cadencia rígida; los ojos despiertos son óvalos rellenos y el parpadeo los comprime verticalmente hasta una cápsula fina, sin ocultarlos. El mismo contorno relleno abre/separa los párpados al despertar y vuelve a la curva cerrada al dormir, sin fundir ni reemplazar dibujos. El parpadeo se atenúa durante la transición, desaparece al dormir y movimiento reducido detiene todo el idle. No se añade lógica de negocio ni cambios de foco. El seguimiento del cursor añadido después se documenta abajo.

Verificación específica: compilación declarativa completa de presence.slint con Slint 1.18.1 y salida temporal correcta; invariantes y diff sin errores. Había otro proceso Cargo construyendo dictado en el target compartido, por lo que no se lanzó una campaña Cargo paralela. No se arrancó ni reinició la instancia del usuario; suavidad visual en ejecución pendiente de evaluación.

## Apertura continua de ojos — 2026-10-11

Se retira el fundido entre curva dormida y óvalo despierto: cada ojo usa un único Path relleno con cuatro curvas Bézier. Los bordes superior/inferior se separan progresivamente, la curvatura dormida se relaja y el contorno se convierte en el óvalo abierto. El parpadeo comprime ese mismo contorno manteniendo ancho y centro del ojo despierto. No se alternan elementos ni opacidades. Se conservan las duraciones, idle, movimiento reducido y callbacks de interacción.

Verificación: `cargo build --locked -p soc`, invariantes y `git diff --check` correctos. No se reinició la instancia existente; la apertura continua aún requiere evaluación visual en ejecución.

## Mirada hacia cursor y vidrio — 2026-10-11

La cara despierta sigue la dirección del mouse suavemente, conservando apertura y parpadeo continuos. macOS lee cursor global junto al hit test; Winit ofrece fallback local en otros OS. Dormido/movimiento reducido centran la mirada. Material en ambos estados dibujado con Slint: translucidez, reflejos, gradientes y bordes. Aproximación de vidrio portable sin controles macOS, sin blur/refracción real del escritorio. Véase [regla vigente](../rules/frontend.md#mirada-y-vidrio-portátil) para capacidades y fuentes.

Verificación específica: `cargo test --locked -p soc-presence` correcto (4 pruebas, incluida dirección/rango de mirada), compilación Slint correcta, invariantes y diff correctos. Se formatearon solo los archivos Rust editados. No se reinició la instancia del usuario ni se certifica aquí seguimiento visual del cursor/transparencia compositada; otros OS y monitores pendientes de evaluación.

## Ejecutar y verificar
`cargo run --locked -p soc` desde raíz. `.env` local preparado con SOC_DATABASE_PATH=data/soc.sqlite3, SOC_DB_BUSY_TIMEOUT_MS=500; no secretos/puertos. Clic alterna; el menú Acerca de soc abre información y el menú Salir termina. M, A, Espacio, Escape y cierre con clic derecho están desactivados. Ver [README](../../README.md).

Pasaron `cargo fmt --all -- --check`, `cargo test --workspace --locked` (13), `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --locked -p soc` y `python3 scripts/check_invariants.py`. Tests recorren atomicidad/checksum/historia de migración, reapertura real, namespaces, cierre compartido, registro y orphans, DAG, dependencias y selección, fallo de startup y cleanup inverso, lifecycle idempotente, precedencia de configuración, variables sincronizadas, commit y error real de worker.

## Verificación del menú de barra superior — 2026-10-11
Pasaron formato, 13 pruebas de workspace, Clippy con warnings denegados, build locked y comprobación de invariantes. Ejecución aislada con SQLite temporal confirmó status_menu_ready/ui_ready sin errores de creación nativa. Un paquete .app temporal permitió inspeccionar la cara dormida mediante accesibilidad y captura, pero la herramienta no expuso los iconos de barra superior: apertura, acciones del submenú, Acerca y cierre desde el icono quedan pendientes de comprobación manual. No se sustituye esa evidencia por pruebas unitarias. Instancias propias de evaluación cerradas, sin detener la instancia existente del usuario; no se publica ni instala el paquete temporal.

## Evidencia runtime y límites
La evidencia siguiente corresponde a la base inicial, anterior a la desactivación de atajos y ayuda. Este cambio requiere evaluación manual actual; compilación y pruebas no demuestran comportamiento runtime.
Instancias locales con SQLite aislado en .verification; versión final arrancó y cerró con module_stopped/shutdown_complete. Ventana dibujada en macOS, caras y etiquetas de estado, zetas cambiantes y tamaño de expansión en capturas sucesivas, M confirmado después del commit mediante SQL reduced_motion=1, reapertura de ese archivo sin reaplicar migraciones. A abrió AboutSlint 1.18.1 con controles legibles. Native readiness reportó monitor 1920x1080 scale=1 y posicionamiento inferior derecho; el menú se declara pero no se recorrió por UI.

Captura de app aislada muestra exterior claro y no demuestra transparencia compositada. Las acciones de automatización se dirigen a apps concretas; un contador en la ventana inferior respondió, incluso al dirigirse a su zona cubierta, por lo que no se usa ese resultado para certificar click-through/z-order. Geometría circular tiene unit test y macOS set_cursor_hittest sigue diámetro cada 16 ms; requiere prueba manual de paso de eventos y reentrada rápida. Foco de otras apps, no robo de foco, always-on-top respecto de ellas, Dock, varios monitores, Retina y otros OS pendientes de evaluación real. El fallo posterior de pertenencia a Spaces normales fue corregido y confirmado por el usuario según [D-014](../decisions/spaces.md); fullscreen y Stage Manager siguen sin validación. Capturas temporales confirman cambios, sin revisión audiovisual continua de suavidad. Otros OS no tienen adaptación de click-through todavía.

## Documentos y origen
[Arquitectura](../architecture/overview.md), [mapa](../architecture/modules.md), [reglas](../../AGENTS.md), [requisitos](requirements.md), [decisiones/versiones/avisos](../decisions/README.md), [memoria](../memory/index.md). Dependencias revisadas oficialmente; aviso transitivo de mantenimiento bincode en grafo macOS, no certificación de seguridad. .creation queda histórico, prompt intacto byte por byte. La inicialización se realizó sin commit/push ni acceso remoto; la [entrega Git posterior](../memory/2026-10-11-git-delivery.md) tiene autorización independiente. No procesos temporales vivos.

## Plan original y evolución conservados

# Plan de inicialización

Estado: inicialización completada; archivo histórico. Fuente vigente en docs; cierre y límites al final.

## Último paso completado
- Comprobar que no existía .creation en el destino indicado.
- Conservar el prompt íntegro y crear answers.md, decisions.md y plan.md.
- Verificar lectura y coincidencia byte por byte del prompt con el adjunto.

## Siguiente paso pendiente
- Nombre recibido: soc. Recoger función, necesidad y público de la aplicación.
- Resolver la carpeta definitiva dentro de Dev y registrar la respuesta antes de avanzar.

## Alcance de la base
Pendiente de requisitos. No se han creado código, workspace, dependencias, frontend ni migraciones.

## Checkout
Pendiente determinar si existe repositorio y checkout, su ruta, rama y remoto. No clonar sobre la carpeta ocupada por .creation. Si corresponde trasladar la documentación al destino final, conservar el contenido y verificar las nuevas rutas. No hay autorización de commit, push ni publicación.

## Continuación
Leer initialization-prompt.md, answers.md, decisions.md y plan.md antes de generar la base o retomar esta inicialización. Cuando se cree AGENTS.md, incluir allí esta condición de lectura.

## Construcción y verificación
Pendiente definir producto y decisiones estructurales, escribir propuesta vinculada a requisitos y verificar dependencias antes de incorporarlas. Consolidar los documentos al terminar según el prompt.

## Síntesis inicial del producto — 2026-10-11
soc será un asistente personal orientado a productividad y rapidez de decisión, proactivo e integrado con el sistema operativo. El usuario desea aprovechar cuotas de servicios que ya paga.

Último paso completado: registrar el propósito y separar objetivos de integración de capacidades todavía no verificadas.
Siguiente paso: elegir el sistema operativo inicial, concretar los recorridos de la primera versión y definir autonomía. Después investigar las integraciones admitidas por los proveedores elegidos.
No se ha decidido stack de interfaz, motor de datos, conectores ni módulos; no generar código hasta resolver las decisiones estructurales.

## Avance — plataformas, 2026-10-11
Último paso completado: registrar macOS como primer hito y la orientación multiplataforma.
Siguiente pendiente: recoger tareas concretas del primer hito, decidir límites de autonomía y luego derivar las integraciones necesarias. La portabilidad debe orientar los límites del diseño sin prometer soporte no probado ni generar módulos sin requisitos.

## Alcance vigente del primer hito — 2026-10-11
- Prioridad: base gráfica evaluable; construcción incremental junto al usuario.
- Soporte técnico: ejecución local, SQLite y kernel mínimo vinculado a requisitos reales.
- Exclusiones iniciales: grandes funcionalidades de dominio, usuarios y sesiones.
- Adaptar los patrones de persistencia del prompt a SQLite; no generar infraestructura PostgreSQL.
Último paso completado: registrar alcance y prioridad gráfica.
Siguiente paso: decidir el tipo de interfaz; concretar referencia/forma visual, carpeta final y Git antes de ejecutar la construcción.
Las tareas de autonomía y conectores con proveedores pertenecen a fases posteriores, salvo nuevo alcance explícito.

## Avance — forma gráfica, 2026-10-11
Último paso completado: registrar interfaz nativa sin WebView y círculo flotante con expansión/contracción.
Siguiente paso: evaluar Slint para transparencia, ventana sin marco y animaciones; definir activación manual de la demostración y política de posición/foco. No integrar correo ni voz en el primer hito sin cambio explícito de alcance.

## Avance — estados y propiedad de interfaz, 2026-10-11
Último paso completado: registrar cara siempre visible, reposo dormido con zetas, estado activo contento y expandido, siempre encima y demostración por clic.
Una app dedicada poseerá la interfaz; bootstrap será el punto de composición y kernel contendrá la infraestructura independiente de esa presentación. Evitar rutas HTTP, usuarios y sesiones sin requisitos.
Siguiente pendiente: concretar carpeta final y situación de repositorio Git; validar Slint en macOS antes de fijar dependencias y completar la propuesta de base. Propuesta gráfica inicial: clic alterna reposo/activo, con transiciones suaves; duración, color, tamaño y esquina son detalles por definir.

## Avance — preparación de Git, 2026-10-11
Último paso completado: registrar que el usuario creará el repositorio y entregará la URL.
Siguiente pendiente de checkout: recibir dirección sin credenciales, confirmar estado del repositorio, ruta final y autorización de clonado si hace falta. Conservar .creation y no clonar sobre la carpeta ocupada.
Trabajo de descubrimiento independiente posible: definir estilo de la cara, color y ubicación inicial; continuar evaluación documental de Slint.

## Avance — dirección del repositorio, 2026-10-11
Último paso completado: registrar git@github.com:ElHaban3ro/soc.git sin credenciales.
Siguiente pendiente: preguntar si ya está clonado o si se autoriza clonarlo en Dev/soc; después verificar checkout y rama, preservar contenido existente y trasladar la documentación inicial verificando integridad. No asumir rama main.

## Estado vigente — checkout preparado, 2026-10-11
Último paso completado: crear /Users/fer/Documents/Projects/Fer/Dev/soc, inicializar Git, configurar origin y trasladar .creation verificando integridad. Rama inicial: main; sin commits ni push.
Fuente vigente de inicialización: /Users/fer/Documents/Projects/Fer/Dev/soc/.creation. Las ubicaciones anteriores quedan como historial.
Siguiente pendiente: cerrar la evaluación de Slint y proponer la base gráfica y kernel mínimo con versiones verificadas, decisiones y criterios de ejecución. Mantener la construcción incremental solicitada.

## Estado vigente — orden corregido y animaciones, 2026-10-11
Último paso completado: registrar que kernel y SQLite forman parte de la primera construcción y no se aplazan tras una demo aislada.
Siguiente pendiente inmediato: evaluar soporte de animaciones suaves y rebote/resorte de Slint; informar capacidades y límites antes de fijar el toolkit. Después preparar e implementar base conjunta con dependencias verificadas y documentación aplicable.

## Ejecución autorizada — 2026-10-11
La base conjunta está autorizada. Primer alcance: workspace Rust con kernel de configuración/persistencia y contratos mínimos de módulos, app dedicada de interfaz Slint y bootstrap; SQLite compartido y migraciones necesarias; presencia sin marco siempre encima, cara dormida/activa, clic y transiciones. Sin HTTP, usuarios, sesiones, voz, correo ni conectores de IA en esta fase.
Antes de incorporar dependencias: verificar versión exacta, compatibilidad, features y avisos en fuentes oficiales. Validar comportamiento real de transparencia, foco, expansión y cierre en macOS y distinguirlo de compilación/pruebas.
Bloqueo de enrutamiento: soc no figura entre los proyectos disponibles de Codex. La ejecución del repositorio requiere un chat de su proyecto; falta registrar/abrir soc como proyecto en la aplicación. No dirigir trabajo a iClock ni a otros proyectos.
Último paso completado: registrar autorización y alcance conjunto.
Siguiente pendiente: disponer del proyecto soc en Codex para enviar la implementación con todo el contexto y supervisar hasta entrega.

## Propuesta vinculada y construcción — 2026-10-11
Estado vigente: construcción en el checkout soc; el impedimento de enrutamiento anterior está resuelto por este chat.
- R1 nativa/cara: apps/presence posee UI Slint, estados y movimiento; clic alterna, Escape/clic secundario sale. 80 px dormida y 168 px activa, cara siempre visible, azules #b8dcf5/#74b6e8, tinta #20394d, mejillas #f1a9b7 y zetas #86a4bf. Tipografía del sistema para zetas y ayuda; cara dibujada con paths, sin imágenes externas. Esquina inferior derecha y margen 24 px: supuestos delegados revisables.
- R2 kernel inicial: kernel/config carga Config y Secrets separados; kernel/db posee una conexión SQLite compartida/serializada, busy timeout, foreign keys, WAL y migraciones transaccionales con checksum SHA-256 por módulo. kernel/core define AppModule, AppState y apagado persistente. Sin pool artificial para una app local con escrituras mínimas.
- R3 persistencia justificada: registro modules conserva activación entre arranques; presence_preferences recuerda reducción de movimiento (tecla M), no el estado activo transitorio. Arranca dormida. SQLite archivo local data/soc.sqlite3 explícito en .env; sin secretos necesarios hoy.
- R4 composición: bootstrap registro explícito, validación DAG antes de migrar apps, migración de módulos compilados incluso apagados, start/stop idempotentes y limpieza inversa. El bucle gráfico corre en main y queda fuera de start(); sin HTTP, hot activation ni buses sin consumidores. No contracts/testkit vacíos.
- R5 evaluación: fmt, pruebas SQLite en memoria/archivo temporal (rollback, checksums, reapertura, registro, dependencias y lifecycle), clippy/build y UI macOS cuando el entorno permita observarla. Transparencia, posición, foco, click fuera y animaciones se reportan según evidencia real.
- Dependencias propuestas verificadas en crates.io/mantenedores: Slint/slint-build 1.18.1 (MSRV 1.92; Rust local 1.95), rusqlite 0.40.2 bundled/defaults off, dotenvy 0.15.7 y sha2 0.11.0. Winit vía reexport de Slint y feature unstable-winit-030 para política de ventana. Licencia Royalty-free para desktop con atribución AboutSlint accesible (tecla A). Avisos RustSec históricos rusqlite/sha2 corregidos; ausencia de directorio para Slint/dotenvy no certifica seguridad. Revisar lockfile transitivo al resolverlo.
Último completado: lectura íntegra de documentos y checkout sin código previo. Siguiente: generar workspace funcional y reglas, ejecutar verificación.

## Ajuste vigente — 2026-10-11
Corrección del usuario: cara monocromática, moderna y minimalista. Supuesto azul/rosado reemplazado por grafito #242424, blanco suave #f2f2f2 y grises #555555/#a3a3a3; sin mejillas. Conservar historia, actualizar fuentes vigentes y verificar visualmente.

## Cierre de la primera base — 2026-10-11
Estado vigente: inicialización completada, con límites de validación runtime documentados. Fuentes vigentes: docs/initialization/requirements.md, docs/initialization/implementation.md y docs/decisions/README.md. Este archivo conserva origen e historial, no es una segunda fuente vigente.
- Base construida: kernel/config, kernel/db, kernel/core, apps/presence y bootstrap. SQLite compartido, migraciones atómicas con SHA-256 por módulo, registro/DAG y cierre inverso.
- Cara monocromática minimalista conforme a D-013. Clic/Espacio, zetas, spring/contracción, M persistido, A y menú Acerca, Escape/clic secundario/menú Salir.
- Verificación final: fmt, 13 tests, Clippy con -D warnings, build y check_invariants verdes. UI observada en macOS; M confirmado por lectura SQLite y reapertura sin migraciones repetidas.
- Límites: capturas secuenciales no equivalen a revisión continua del movimiento. Foco de otras apps, transparencia compositada, always-on-top y paso real de clics requieren comprobación manual OS; automatización app dirigida no demuestra esas garantías. Multi-monitor/Retina/Spaces/otros OS no probados. Click-through específico macOS, no implementado en otros OS.
- Prompt SHA-256 original conservado: 7412501ba941c357418096079a62cff4487dccf9cb138f197c36f4088ea3d9dd. Sin commit/push, rama main y origin conservados, sin procesos temporales vivos.
Último paso completado: construir/verificar/consolidar. Siguiente: evaluación manual de comportamiento de ventana y feedback del usuario; no hay bloqueo de configuración ni de compilación. Conectores y autonomía quedan fuera de este hito.

## Reubicación por arrastre — 2026-10-11
La cara contiene WindowMoveArea de Slint 1.18.1 alrededor de TouchArea: botón izquierdo, umbral nativo de arrastre y clic independiente. Winit realiza el movimiento de la ventana; no hay nueva dependencia, worker ni escritura de posición. Cursor de mano y etiqueta accesible indican la interacción. Al reiniciar vuelve a la ubicación inicial; los atajos suspendidos siguen suspendidos.

Verificación en copia temporal de kernel/config, kernel/db, kernel/core, presence y bootstrap: build, 13 tests y Clippy pasaron. Se usó target separado; no se cambió el lockfile compartido porque el workspace recibía simultáneamente apps/transcription. Esto no certifica la compilación de esa app concurrente. Durante la revisión también cambiaron las animaciones de presence en otro trabajo; se preservaron junto al área de arrastre.

Instancia macOS con SQLite propia: cara dormida/activa y clic posterior confirmados mediante accesibilidad. Los arrastres sintéticos mantuvieron el estado, pero no movieron la ventana (posición nativa observada 1672,744): no se declara arrastre físico validado. Se observó posición solo con instrumentación en la copia temporal, nunca añadida al checkout. Se cerró la instancia mediante su menú Salir. Pendientes prueba física, reentrada del hit test después de mover, Retina y varios monitores.

## Corrección del límite superior de arrastre — 2026-10-11
El usuario reprodujo un tope cercano a media pantalla. La sonda de NSWindow con marco 400x460 y cara de 80 px a 32 px del borde inferior reprodujo minimum_face_top=378, código 1, con la política original. El margen invisible superior de la ventana, no la cara, era el límite de macOS.

Se conserva WindowMoveArea y se adapta constrainFrameRect:toScreen: solo para la identidad de la presencia, conservando clase/ivars de Winit y el comportamiento de otras ventanas. El registro se retira al salir. No se añadió movimiento manual, persistencia ni cambios a las animaciones y transcripción concurrentes. La sonda de regresión es una ventana mínima con esas dimensiones y el desktop.rs real, sin DB/audio; comprueba política, posición, otra ventana, idempotencia y retirada. No certifica arrastre físico ni varios monitores/Retina.

Resultado de la regresión nativa: minimum_face_top=0, positioned_face_top=0 y window_top=-348; otra ventana y retirada de identidad conservaron 378 px. Pasaron 20 pruebas del workspace y build locked en el checkout compartido; Clippy de presencia/all-targets pasó en la copia con target separado. Formato de archivos afectados, invariantes y diff sin errores. No se detuvo la instancia del usuario; requiere reinicio para aplicar la política. La regresión prueba el límite/posición nativos, no un arrastre físico ni Retina/multi-monitor.

## Extensión posterior D-015: transcripción
Se añade dictado local Nemotron con atajo configurable (mantener/alternar), globo progresivo animado y borrado temporal, configuración Slint y descarga verificada con aviso. Implementación y evidencia independientes de la primera base: [decisión](../decisions/transcription.md), [verificación](../verification/transcription.md). No guardar voz/transcripción ni pegar texto.

## Arrastre real: corrección posterior — 2026-10-11
El usuario confirmó que el tope persistía tras la política de NSWindow: la sonda anterior solo probaba posicionamiento y no todo performWindowDragWithEvent. Había dos procesos: target/debug/soc y .verification/soc-transcription-review.app/Contents/MacOS/soc; no se cerraron ni reemplazaron sus datos.

En macOS se deshabilita WindowMoveArea y se usa DragGesture con cursor global y botón físico, en el timer existente y callbacks de la cara. Se conserva la política de marco negativo para el setter. Captura de mouse durante pulsación, terminación aunque falte pointer-event up y supresión del clic tras mover; nueva pulsación reinicia. No se acepta captura sintética sin botón físico, para impedir movimiento residual. La mirada y el click-through vuelven a su comportamiento normal al soltar.

La sonda ahora recorre muestras del controlador: cara a 460, 260 y 0 px del borde superior; soltado y siguiente clic comprobados. Prueba de UI aislada detectó pérdida de soltado en eventos sintéticos; esa versión se descartó y se añadió el control físico del botón. La interacción física final queda pendiente; no repetir la afirmación de arrastre real corregido basándose solo en setters. Paquete temporal propio cerrado, sin alterar procesos del usuario.

Verificación final de esta revisión: 22 pruebas de workspace, Clippy workspace/all-targets con warnings denegados y build locked pasaron. Formato, invariantes y diff correctos. Sonda de muestras del controlador llegó al borde y comprobó soltado/siguiente clic; queda pendiente prueba física de la versión final. Las dos instancias previas se conservaron; el nuevo target/debug/soc está compilado.

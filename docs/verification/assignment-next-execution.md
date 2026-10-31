# Próxima ejecución en la lista de Encargos

29 de octubre de 2026. Preparación `.verification/assignment-next-execution/56d8dd13-8520-4f2b-a6c2-74d8143a6bb6`. Cumple el requisito del plan de mostrar próximo evento y estado en la lista. El goal sigue activo; sin commit/push, instalación o despliegue.

## Comportamiento y límites

Assignments proyecta la fecha persistida más cercana de sus políticas habilitadas, activas y vigentes para hasta cien encargos visibles. Comprueba instancia, revisión y documento activo con la misma conexión SQLite; no calcula otro horario, prepara trabajos ni ejecuta funciones. Excluye políticas pausadas, documentos editados u otra instancia. Una fecha activa inválida produce error, sin inventar un valor. No se añade ni modifica SQL histórico.

Bootstrap expone `AssignmentRecord.next_execution_at` opcional mediante FRB regenerado. Flutter presenta fecha completa, año, hora local del equipo y estado. «Prevista» expresa planificación guardada, sin garantizar ejecución. Sin fecha conserva tipo/estado. El subtítulo previsto permite hasta tres líneas y tinta existente al 80 % para mantener legibilidad al mínimo y con texto ampliado; los demás SidebarTile conservan sus valores anteriores. No cambia la barra vertical, tema, material, iconos ni ancho de navegación.

El buscador estrecho al 150 % envolvía su placeholder en dos líneas dentro de una caja de 40 px. Una regresión reprodujo el recorte. Mantener una línea con elipsis lo corrige sin alterar altura, tipografía, color o nombre accesible completo; no se emite búsqueda al presentarlo.

## Evidencia automática y visual

Tres pruebas del Store cubren mínimo entre políticas, pausa, documento obsoleto, reutilización de ID con otra instancia, límites, fecha inválida y lectura sin efectos. Otra de runtime verifica snapshot, persistencia/reapertura y retirada tras pausa, sin recalcular el calendario. La regresión UI reproduce ausencia inicial, muestra la fecha recibida aunque difiera del horario del documento y la retira al actualizar el snapshot.

Seis casos macOS cubren claro/oscuro, 1100 × 720/sidebar 260, 640 × 420/sidebar 216 y mínimo con texto al 150 %. Fecha/hora/estado no exceden las tres líneas. Se inspeccionaron ambas capturas mínimas ampliadas finales. Contraste software: 6,54506:1 en claro y 9,11259:1 en oscuro. Doce imágenes completas de chat/detalle son idénticas en RGBA antes/después; cubren esas superficies, no las filas deliberadamente modificadas.

Los registros rojos de ausencia/recorte se conservan. El primer muestreo capturaba un boundary transparente sin su fondo y no era válido; se corrigió el fixture incluyendo Scaffold y muestreando dentro del Material, fuera de letras. La medida posterior válida al 62 % dio 3,86314:1 en claro y motivó el 80 % solo para la fecha prevista. La medida inválida no acredita un fallo del material nativo.

Resultados finales: Rust 419 correctas/28 optativas omitidas, Clippy workspace/all-targets sin warnings y formato correctos; Flutter 40 afectadas y suite 393/49, análisis limpio, release y firma local correctos. `RUST_VERIFICATION.json`, `FINAL_FRONTEND_VERIFICATION.json`, `VISUAL_VERIFICATION.json` y logs conservan cada fase.

## Recorrido nativo aislado

Release habitual SHA-256 `1424528f55ee5aa570c4ccbb8df9414e2954aeb1f556bf6ccce7c3827e6a175d`; copia con bundle UUID, PID 77510, ventana normal, sin tema forzado o preferencias del sistema. Presence/Assignments activos; conversación, guía, Plugins, navegador y audio apagados, sin Runner. Cuatro encargos/ocho políticas y cuatro intentos inciertos precargados ficticios; fechas activas en 2099.

La lista mostró únicamente «Ejecución ficticia 1 — Prevista: 01/01/2099 08:00 · Activo». Las otras filas, con política pausada, revisión obsoleta y encargo pausado respectivamente, no mostraron fecha. Abrir el primero confirmó Preparación activa y la misma fecha local. Pausar retiró inmediatamente la fecha de la lista y mostró Preparación pausada/Sin próxima ejecución. Capturas y AX están en la traza Computer Use.

Salir por menú dejó el PID en Z antes de recoger el padre; producto y script terminaron con 0. `run-next-execution/manifest.json` y el guard final confirman integridad ok/FK vacías, documentos e historial/perfil iguales, ocho versiones anteriores intactas y nueva versión de preparación con idénticos valores; únicamente preparation-1 pasa a revisión 2, deshabilitada/pausada/sin fecha. Permanecen cuatro intentos precargados, cero trabajos, mensajes o nuevas ejecuciones. Fixture/preferencias propios retirados, sin errores observados de motor/canal. Debug 49455 conservado.

No acredita VoiceOver, modificadores físicos, contraste cuantitativo del material nativo o toda la matriz de ajustes reales. La aprobación de accesibilidad sigue pendiente. La campaña 1e46eae9 se cerró antes de live al cambiar fuentes/API/helper sellados; cero turnos/guía, sin reutilizar permiso o máximos. La siguiente campaña requiere preparar y revisar las fuentes finales.


## Preparación de modelos posterior

La sustituta c4462341 también se cerró antes de live. Su primer protocolo sin cuenta falló al escribir un fingerprint de libsqlite3-sys: la carpeta de target/flutter/debug ya no existía. Se observó otro Cargo, PID 84537, con cwd de soc y archivos de ese target abiertos; no se le detuvo ni se atribuye sin prueba la eliminación de la carpeta. El helper sellado estaba ausente y no había deriva de fuentes. No se reintenta ese protocolo o preparación. Otra preparación preliminar seleccionó por error diez casos al usar scope full y fue cerrada sin ejecutar; scope pilot conserva los cuatro casos y el presupuesto 16/8 previsto. Todos los cierres/logs están conservados; cero modelos/guía. Antes de pedir otra aprobación deben estar libres el target y estable el helper. Esto no renueva ni invalida las pruebas UI/nativas anteriores de su release documentado.


El otro Cargo terminó después. Se reconstruyó el helper con ./scripts/dev.sh --build-only (salida 0, sin abrir la UI), y la campaña 87b05239-f97a-4b0f-a1d4-fd3f36cc3c60 pasó cuatro comprobaciones nuevas sin cuenta: protocolo Detener/reanudar, protocolo plugins, baseline de 22 filas y protocolo de una celda de Encargos. Cero checks falsos/modelos/guía, 213 fuentes/binarios sellados y helper coincidente entre etapas. Su plan conserva cuatro casos y máximo conjunto 21 turnos/8 guía/25 minutos de fases más compilación/cierre. Aprobación exacta solicitada; sigue pendiente. HELPER_RECOVERY.json conserva el helper y la recuperación, sin borrar el fallo previo.


## Seguimiento: mínimo claro y oscuro con texto ampliado

Preparación `.verification/native-next-execution-minimum/be5f937a-945a-4142-b5bd-6f26b323439e`. Copia privada de las 98 fuentes/manifiestos del frontend, cambiando únicamente main.dart y MainFlutterWindow.swift: inicio a 640 × 420, texto 150 %, highContrast/disableAnimations de MediaQuery y selección coordinada del tema Flutter/apariencia AppKit clara u oscura por variable QA. No cambia preferencias del sistema, el producto o su backend. Biblioteca/helper copiados del release habitual, backend idéntico por hash y firma local verificada. La selección de tema QA no prueba notificaciones reales del sistema.

En oscuro, PID 13563, el agarre existente permitió estrechar la sidebar a 216 px (límite visible en x=276, después de los 60 px de navegación). Desplazar la lista mostró fecha completa/año/hora/estado en tres líneas; solo Ejecución ficticia 1 tenía fecha vigente. Buscar un texto ficticio sin coincidencias mostró el estado correspondiente y Limpiar búsqueda recuperó las cuatro filas. El placeholder se mantuvo en una línea con elipsis al estrechar. Abrir la ficha y desplazarla permitió alcanzar Preparación/Activa y Pausar ejecuciones; después mostró Preparación/Pausada/Sin próxima ejecución y retiró la fecha de la lista. Título y barra existentes conservados.

El primer claro, PID 16314, mostró Encargos y las cuatro filas, pero las acciones posteriores de arrastre/scroll no se confirmaron; Computer Use devolvió elementHasNoFrame y dos timeoutReached al pedir el árbol completo. El PID seguía vivo: no se reinició por un timeout de observación. Una muestra de un segundo capturó el hilo principal de AppKit esperando eventos, sin probar por ello ausencia de cualquier fallo. Salir por teclado respondió, PID Z confirmado y producto 0. El guard del driver falló con 1 porque la pausa no se había realizado; el timeline completo permaneció exactamente igual. Se conserva como recorrido fallido, no como aprobación de esa matriz o defecto demostrado del producto. Causa de la pérdida de observabilidad sin determinar.

Tras cerrar esa copia y renovar la sesión de Computer Use, otra base/bundle, PID 21185, completó el recorrido claro con el mismo binario QA, sin cambios de código. Búsqueda sin coincidencias/Limpiar, agarre a 216 px, scroll para fecha completa en tres líneas, acceso a Preparación y pausa con retirada inmediata de fecha. Las capturas de 640 × 420 y árboles AX están en la traza Computer Use, sin PNG exportado ni contraste nativo numérico. El resultado posterior no sustituye el manifiesto del primero.

Las lecturas ro antes de pausar, tanto oscuro como claro final, comparan el timeline completo con el inicial: buscar, desplazar y redimensionar no escribieron en SQLite. En ambos casos satisfactorios, Salir por menú, PID Z antes de recoger el padre, producto y driver 0; integridad ok/FK vacías, documentos/historial/perfil y ocho versiones anteriores iguales. Solo preparation-1 pasa a revisión 2/deshabilitada/pausada/sin fecha y añade la versión con idénticos valores. Cuatro intentos precargados, sin otros nuevos, cero trabajos/mensajes/hilos/modelos/guía y sin errores observados de motor/canal. Los tres fixtures/bundles/preferencias se retiraron.

NATIVE_OBSERVATIONS.json y CLOSURE.json separan los tres procesos y sus resultados. Fuentes originales y QA, diferencias de los dos archivos, logs y muestra de proceso archivados; proyecto/paquete QA retirados. Las 98 fuentes y el ejecutable habitual SHA-256 1424528f55ee5aa570c4ccbb8df9414e2954aeb1f556bf6ccce7c3827e6a175d permanecieron iguales; Debug 49455 siguió vivo. Campaña 87b05239 con sus fuentes/binarios coincidentes y aprobación todavía pendiente; cero inferencias. No se repiten suites sin cambios de producto; siguen vigentes 419 Rust/28 y 393 Flutter/49 de la etapa anterior.

Esto amplía la matriz nativa de la lista/fecha/búsqueda/ficha/pausa en condiciones QA. No acredita VoiceOver, modificadores físicos, transparencia reducida del sistema, todos los formularios o estados posibles ni una medición cuantitativa de contraste del material nativo. Se conserva el mensaje vacío exacto acordado en UI.md, sin reintroducir texto promocional. Goal activo; sin commit/push, instalación o despliegue.

La identidad por hash de la biblioteca backend QA quedó registrada en BUILT.json. El helper se copió del release mediante el script, pero no se conservó un digest individual de esa copia; los módulos de audio permanecieron apagados. No se presenta este recorrido como verificación de ejecución del helper.


Estado posterior de modelos: 87b05239 fue autorizada y cerró tras tres turnos/0 guía, con Detener/reanudación correctos, plugins sin resultado final evaluable y Encargos sin live. Se corrigió la pérdida de diagnóstico de la sonda sin nuevas inferencias; esa campaña y sus máximos sobrantes no se reutilizan. [Informe](../../.verification/model-campaign/87b05239-f97a-4b0f-a1d4-fd3f36cc3c60/REPORT.md). Los recorridos UI anteriores conservan su versión y los controles cfg(test) nuevos no cambian el ejecutable habitual.

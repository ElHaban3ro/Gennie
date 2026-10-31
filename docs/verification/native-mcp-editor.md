# Editor MCP nativo en oscuro y texto ampliado

29 de octubre de 2026. Preparación en .verification/native-mcp-editor/185a2968-1dea-420c-b234-e0c65db45275. Copia QA de 97 fuentes/manifiestos, con ventana inicial 640 × 420, escala 1,5, highContrast y disableAnimations dentro de MediaQuery. ThemeMode.dark selecciona el darkTheme existente y darkAqua la apariencia de esa app. No se cambian preferencias del sistema. Los cambios QA están restringidos a main.dart y MainFlutterWindow.swift; el backend coincide por hash con el release habitual.

Plugins, Presence y Assignments están activos solo en la base ficticia; navegador, conversación, modelos, voz y guía están apagados. Plugins tiene dependencias opcionales y puede guardar fuentes sin conversación. Browser.call rechaza antes de crear sesión/leer catálogo cuando su módulo está apagado; MCP save usa SQLite y no crea un cliente. No se consulta el catálogo personal, conecta un servidor, descubre herramientas ni ejecuta efectos externos para este recorrido.

## Primer recorrido: observaciones y cierre fallido

Run 389fa2f9-6351-4ec3-9fa5-7a9ac51aae81. Nuevo plugin → Conectar un servidor MCP existente → editor HTTP. Guardar sin nombre llevó al campo y mostró «Completa Nombre.»; una vez indicado el nombre, Guardar llevó a URL y mostró su error. «invalid» produjo la explicación de dirección no admitida. Los errores y el footer Guardar conexión/Cancelar permanecieron visibles a tamaño mínimo/150 % con scroll del cuerpo.

Se guardó únicamente MCP de prueba con https://example.invalid/mcp: revisión 1, deshabilitado, herramientas/permisos vacíos, checked_at=null y cero intentos/trabajos/ejecuciones. El perfil, los dos mensajes y revisiones 1/1 de Encargos se conservaron. No se pulsó Descubrir ni se concedieron permisos. La ficha presentó Desconectado.

Un clic AX sobre el nombre tras el scroll abrió el selector de transporte; se cerró y se usó el campo visto en la captura. Algunas instantáneas AX conservaron un valor incompleto del nombre mientras la imagen y el documento SQLite coincidían en MCP de prueba. Se registra como limitación de observación/input; no acredita VoiceOver ni demuestra por sí sola una causa en el producto.

Al alcanzar el límite, el harness terminó la instancia con -15. El intento de Salir no acreditó una acción ejecutada. Integridad/FK y retirada de fixture/preferencias correctas, pero **el recorrido no pasa cierre limpio**. BEFORE_OBSERVATION.json y manifiesto se conservan; no se convierten en una ejecución aprobada.

## Arreglos derivados

La ficha exponía «Editar conexión MCP» en un contenedor, separado del botón anónimo que llevaba la acción. La regresión del nodo nombrado reprodujo que no era un botón activable. AppDetailHeading ahora une solo el subárbol de ese control mediante MergeSemantics, conservando callback, dimensiones y estado oculto cuando no se permite editar. El arreglo se comparte con las fichas de Encargos y plugins web.

Recargar plugins tampoco exponía un nombre accesible en su botón. Una segunda regresión falló al buscar el nodo nombrado. El Icon de ese IconButton ahora aporta semanticLabel; el nodo efectivo conserva rol y acción. Los tests activan ambos por SemanticsAction.tap: editar abre el formulario sin guardar/conectar y recargar consulta una sola vez el catálogo sin ejecutar una herramienta.

El aviso de un fallo de Browser se identifica ahora como «catálogo web», para distinguirlo de fuentes MCP disponibles. La regresión existente de MCP con catálogo web inaccesible conserva visibilidad y ausencia de efectos y comprueba esa atribución. No se oculta el error ni cambia su precedencia respecto a errores MCP.

## Evidencia local y límites

Veinticinco pruebas MCP/accesibilidad afectadas correctas, dos regresiones nuevas; suite Flutter 374 correctas/49 optativas omitidas y análisis limpio. Seis combinaciones de claro/oscuro, 1100 × 720/mínimo y escala normal/150 % producen tres capturas cada una: ficha, error y editor. Las 18 imágenes completas coinciden en RGBA antes/después con las mismas fuentes/plataforma/fixtures, usando catálogo web disponible. La atribución nueva del aviso web se verifica por separado; esas capturas no contienen el aviso modificado. No cambian estilos/barra, iconos pintados o materiales.

## Recorrido posterior y cierre

Release habitual y copia QA corregida compilaron y pasaron firma local. El run 0cde5d70-5074-4ac2-950a-182d7191fc25 usa otra base temporal con una fuente ficticia precargada en revisión 1, deshabilitada y sin catálogo. No repite la entrada manual del primer recorrido ni transforma su cierre fallido en éxito.

El árbol nativo presentó un único botón «Editar conexión MCP», además de «Recargar plugins» en el botón efectivo. Se pulsó Editar por su índice nombrado, se abrió el formulario y se guardó una vez conservando la especificación. La ficha pasó a revisión 2, Desconectado y Deshabilitado. Recargar plugins conservó el estado y mostró el aviso atribuido al catálogo web. No se pulsaron Descubrir, Habilitar o Guardar permisos. No se usó fallback de coordenadas en esta repetición. Captura JPEG medida 640 × 420, oscuro/alto contraste/texto 150 % seleccionados en la copia.

Se pidió Salir desde el menú propio unos 38 segundos después de vincular Computer Use. El proceso había terminado antes de pedir al harness su cierre: salida 0, SQLite íntegro/FK vacías y sin errores de motor/canal. Una fuente en revisión 2, especificación ficticia exacta, enabled=0, tools={}, allowed_tools=[], checked_at=null, cero intentos/trabajos/ejecuciones. Perfil/dos mensajes y revisiones 1/1 de Encargos conservados. Fixture/preferencias propios retirados. AFTER_OBSERVATION.json y manifiesto conservan esta muestra separada de la anterior.

Se archivaron fuentes antes/después y los parches exactos. Las únicas diferencias en las fuentes habituales de UI respecto a la preparación son detail_view.dart y plugins.dart; main.dart, materiales, medidas y barra permanecen iguales. La copia compilada/cachés de QA se retiraron después del cierre. La instancia Debug PID 49455 permaneció abierta. El sello de fuentes/binarios de la campaña de modelos 1e46eae9 sigue coincidiendo, sin nueva inferencia ni autorización.

El [seguimiento del probador MCP](native-mcp-probe.md) verifica después descubrimiento/transporte y reapertura en ventana normal, además de probador/revisión mínimos en oscuro y texto ampliado. La creación/edición stdio tiene el seguimiento siguiente a ventana normal. Continúan VoiceOver, teclado físico, eventos reales del sistema y demás estados de la matriz. Las sondas técnicas previas de D-039 conservan evidencia separada de transporte/FRB. El goal permanece activo, sin commit/push, instalación o despliegue.


## Seguimiento: creación y edición stdio a ventana normal

29 de octubre de 2026. Preparación `.verification/native-mcp-stdio-editor/9c81a3e3-0894-4c3b-b8d9-44184210e295`, release habitual 1100 × 720 sin cambios de producto o preferencias. PID 9597, bundle UUID y SQLite propios. Solo Presence/Assignments/Plugins activos; Browser, conversación, guía y audio apagados. No se pulsaron Descubrir herramientas, Habilitar o Guardar permisos. El aviso del catálogo web apagado quedó atribuido al navegador y no impidió guardar MCP.

Nuevo plugin → Conectar un servidor MCP existente → Programa local · stdio. Se introdujeron nombre MCP stdio QA, programa /usr/bin/touch, argumento absoluto a un archivo centinela exclusivo del ensayo, carpeta /private/tmp, nombres ficticios GENNIE_QA_ONE/GENNIE_QA_TWO y plazo 7. No se introdujeron valores o credenciales de entorno. Argumentos `{}` mostraron error local y foco; la lista JSON válida retiró el error antes de guardar. `BEFORE_SAVE_READONLY.json` confirma cero fuentes/escrituras/intentos/mensajes/trabajos/hilos y centinela ausente.

Guardar creó una fuente con especificación exacta/revisión 1, deshabilitada y Desconectado, tools={}, allowed_tools=[], checked_at=null. `CREATED_READONLY.json` guarda el documento completo. Editar reabrió programa, argumentos, carpeta, nombres en líneas separadas y plazo; se cambió nombre a MCP stdio QA editado y plazo a 0. Con avanzadas plegadas, Guardar abrió la sección, desplazó/enfocó el plazo y seleccionó 0 con «Elige entre 1 y 720 segundos.» visible. Reemplazar por 9 retiró el error sin Guardar. `EDIT_VALIDATION_READONLY.json` confirma que la base seguía exactamente en revisión 1. El footer permaneció visible durante el scroll.

La confirmación posterior creó revisión 2 con solo nombre/plazo cambiados y la especificación restante exacta. `EDITED_READONLY.json` y `run-source-editor/manifest.json` exigen perfil/historial/sesión/citas/metadatos y los cuatro encargos/revisiones/propuestas iguales; cero permisos/herramientas/intentos/políticas/versiones/ejecuciones/trabajos/hilos/mensajes. El centinela no se creó antes o después del cierre. Es evidencia de ausencia del efecto de ese programa, junto a guardar como operación de Store sin cliente; no se usa para afirmar ausencia universal de cualquier proceso del sistema. No se inició un servidor ni se verificó aquí transporte MCP: esa evidencia pertenece al probador nativo separado.

Quit Gennie terminó producto/script con 0; integridad/FK y motor/canal correctos, bundle/base/preferencias propias retirados y Debug 49455 conservado. Fuentes y driver archivados; release SHA-256 074eec394111fac8b38d4abd330acc21d7db4efb3cecc0479e92a257c73b0660 y sello de fuentes/binarios/rustc de campaña preparados coincidentes. Sin suites/build repetidos porque no cambió runtime. Capturas/AX en Computer Use; algunos valores AX se observaron desactualizados frente a la captura durante entrada, por lo que las lecturas ro acreditan persistencia. No acredita VoiceOver, modificadores físicos, contraste nativo cuantitativo o matriz mínima/ampliada stdio completa. Cero modelos/guía; autorizaciones pendientes y goal activo, sin commit/push, instalación o despliegue.

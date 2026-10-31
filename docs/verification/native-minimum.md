# Ventana mínima en el renderer nativo

29 de octubre de 2026. Verificación sobre una copia QA del frontend y un runner macOS release compilado en un directorio separado. No cambia el producto habitual, su estilo o navegación. El objetivo de esta comprobación es el contenido nativo a 640 × 420; no acredita arrastrar físicamente la ventana hasta ese tamaño.

## Método y separación del producto

Preparación en .verification/native-minimum/2bdb307d-75ba-448c-a201-656b43a23943. Se copian lib, assets, macos y manifiestos del frontend, excluyendo build, cachés y archivos efímeros. Se registran 94 hashes de origen. En la copia clara, main.dart solo cambia WindowOptions.size de 1100 × 720 a 640 × 420 y añade una lectura diagnóstica del tamaño tras mostrar la ventana. Todos los demás archivos lib son idénticos. El tamaño se verificó mediante las dimensiones del encabezado de la imagen devuelta por Computer Use, no por una afirmación del print.

Flutter pub get --offline, configuración release y Xcode compilan únicamente esa copia. Se reutilizan la biblioteca Rust/helper y licencias del release vigente, con biblioteca idéntica por hash y firma ad hoc comprobada. El paquete habitual tiene ejecutable SHA-256 69896d7046707a0d52be4e33668cf4b896cc50cfcd5a2211a9a99995fa763f3d y no se sobrescribe. No se ejecuta Cargo, cuenta o modelo para esta comprobación.

El harness archivado conserva fixture, límite de 180 segundos, identidad de bundle exclusiva y cierre de native_ui.py; solo cambian la ruta del paquete QA y el import del script. Acciones y capturas mediante Computer Use, con SQLite ficticio y modelos apagados. No se cambian preferencias del sistema.

## Claro: comprobación completada

Run 814da4b1-9d2a-4b6a-806b-6e4981aecfa2, ejecutable QA antes de su firma aislada d74cff7df626c20aa0b710ccf6f81ee84ad20345ed6a282d53c3d19e8e8d85e3. Captura medida de 640 × 420.

- Lector desplazable y compositor visibles, barra lateral existente conservada.
- Perfil abierto desde Opciones → Personalizar asistente: nombre, ayuda y botones Cancelar/Guardar visibles; Escape vuelve al lector sin guardar.
- Cita → original con contorno de foco; tecla Menu, Down y Return abren Responder. Previsualización y cancelación disponibles sin enviar.
- Ficha de Encargos y propuesta abiertas: título, cuerpo desplazable y acciones Cerrar/Descartar/Aplicar visibles; Escape cierra sin resolver.
- Espacio después de una apertura por puntero no reabrió la propuesta. Este recorrido no demuestra restitución desde un invocador enfocado por teclado; conserva la evidencia separada de D-056.

Salir por el menú propio: código 0, sin errores observados de motor/canal, integridad SQLite correcta y FK vacías. Dos sesiones, cita y sesión activa coincidentes antes/después, dos mensajes, revisiones 1/1 y cero trabajos/ejecuciones. Fixture y preferencias QA retirados. La interrupción transitoria de Computer Use con noWindowsAvailable se recuperó seleccionando el mismo bundle; el PID seguía vivo, sin reiniciar la muestra.

## Apariencia oscura: separación de capas

La primera copia añadió solo NSApp.appearance = NSAppearance(named: .darkAqua) antes de crear FlutterViewController. La [propiedad de AppKit](https://developer.apple.com/documentation/appkit/nsapplication/appearance) permite elegir la apariencia de esa app; [darkAqua](https://developer.apple.com/documentation/appkit/nsappearance/name-swift.struct/darkaqua) identifica la apariencia oscura. No cambia la preferencia global.

El run c6b46ebd-0a5c-4e70-a329-47dfa2054af9 siguió mostrando el tema Flutter claro: **no cuenta como verificación oscura**. Cerró con salida 0, perfil idéntico e integridad/limpieza correctas. Esto limita la técnica de QA y no prueba un defecto al cambiar el modo del sistema.

La segunda variante QA selecciona además MaterialApp.themeMode = ThemeMode.dark para renderizar el darkTheme existente. El run 6c9d00fa-d188-4459-99dc-f481ec01053e sí mostró tema Flutter oscuro y captura medida de 640 × 420, ejecutable QA ad31e46805e8abd5dfff072eb77cd860ef25a6863f3862f61daac01adbe6e1f7. Perfil con campos/acciones visibles y Escape correcto; propuesta con título/acciones fijos al desplazar su cuerpo y Escape sin resolver. Salir 0, perfil idéntico, dos mensajes, revisiones 1/1, cero trabajos/ejecuciones e integridad/limpieza correctas. DARK_MODE_OBSERVATION.json conserva el alcance. No acredita notificaciones de cambio de tema del sistema ni la matriz oscura completa.

## Cierre y reproducción

Se comprobaron los hashes de las fuentes y del paquete habituales; permanecieron intactos y la instancia Debug PID 49455 siguió abierta. Se archivaron 98 fuentes/manifiestos en frontend-sources.tar.gz, SHA-256 9f3d9492e7028e2781da3db64acd7559c9a4a95a865e6c6db0fb588412aeffe2. Se conservan qa-main.dart, qa-dark-main.dart, qa-dark-MainFlutterWindow.swift, drivers, logs, versiones de Flutter/Xcode y manifiestos de las tres ejecuciones. La copia compilada completa se retiró después del cierre; no queda un paquete QA activo o instalado.

Para reproducir, crear otra copia privada del frontend desde el archivo, aplicar únicamente la variante QA elegida, resolver los paquetes bloqueados sin actualizar versiones y compilar con la misma secuencia de prepare-build.py. Incorporar biblioteca/helper del producto verificados, firmar y dirigir el harness a ese paquete. Crear siempre otra identidad/fixture de ejecución; no reutilizar registros terminados. La reproducción no modifica el directorio frontend del producto. Esta comprobación no cambió código de producción, por lo que conserva las suites previas y añade evidencia runtime en vez de repetirlas.

## Límites

Las capturas y AX están en la traza de Computer Use; las dimensiones y pasos se registran en LIGHT_OBSERVATION.json, sin PNG exportados. Esto no es una matriz completa de escalas/contraste ni VoiceOver o Mayús físico. Contrastes numéricos de widgets conservan su evidencia separada; no se infieren valores WCAG del material nativo por inspección visual. El goal continúa activo y no se ha publicado, instalado o desplegado el paquete.

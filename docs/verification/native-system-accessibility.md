# Accesibilidad del sistema: contraste observado y ensayos históricos

Estado vigente: el usuario retiró todo trabajo adicional de accesibilidad y contraste. No reanudar ensayos ni tratarlos como requisitos pendientes de cierre. La copia QA activa se cerró y se restauraron las preferencias; continúan solo revisión funcional y entrega. Las notas inferiores conservan su fecha y alcance históricos.

Estado vigente: VoiceOver retirado por instrucción del usuario. No añadir integración, activarlo, reanudar estos ensayos ni pedir observación humana. Lo siguiente conserva únicamente lo ocurrido antes de la retirada; no constituye trabajo pendiente.

29 de octubre de 2026. Pruebas bajo la autorización continuada del usuario, sin nuevos permisos solicitados, inferencias o datos personales. Release de origen `2342cb4a61b4af17f62f025fe0904f5f7ff797e0699f0cfd27ddfeed73d5462c`, copiado y firmado con identidades exclusivas. No se modificó código, estilo o barra lateral.

## VoiceOver

La preparación [f3cdba51](../../.verification/native-accessibility/f3cdba51-e31c-46ab-b410-f8bee6fff52c/PLAN.json) limitó el ensayo a doce minutos y su instancia a ocho. La lectura inicial de Configuración del Sistema mostró VoiceOver apagado. Utilidad VoiceOver tenía ya activado el panel de subtítulos, cursor visible y fuente del panel de 22 puntos; no se cambiaron esas opciones ni se habilitó control por AppleScript.

El interruptor de VoiceOver pasó a encendido. Se elevó la ventana QA, pero no se pudo observar un anuncio o panel de subtítulos. Seleccionar el lector por su identificador devolvió timeout y el inventario informó que no estaba en ejecución. No se interpreta el estado del interruptor como prueba de lectura real ni el fallo de observación como defecto demostrado de Gennie.

Se detuvo el ensayo conforme al plan: VoiceOver volvió a apagado, Configuración del Sistema a General y la utilidad a su categoría General/pestaña Cursor inicial. Cerrar su ventana produjo después procNotFound; no se volvió a abrir. No se cambiaron preferencias de pantalla en este ensayo. El producto QA salió mediante su menú, código 0; driver 0 verifica únicamente SQLite intacta y limpieza. [Cierre](../../.verification/native-accessibility/f3cdba51-e31c-46ab-b410-f8bee6fff52c/CLOSURE.json). **Los anuncios reales y la valoración de VoiceOver permanecen inconclusos.**

La auditoría siguiente encontró otro proceso del mismo bundle temporal, PID 76207, iniciado después del original y adoptado por PID 1. El guard anterior comprobaba solo el PID 74166 y no detectó esta instancia. Se confirmó la ruta exclusiva del fixture antes de terminarla mediante SIGTERM; proceso desaparecido y sin otras copias QA en el inventario posterior. No había rutas abiertas de SQLite/perfil en los metadatos obtenidos al limpiar; eso no permite demostrar retrospectivamente todo su ciclo de vida. [Registro residual](../../.verification/native-accessibility/f3cdba51-e31c-46ab-b410-f8bee6fff52c/RESIDUAL_PROCESS_CLEANUP.json). La causa del relanzamiento no está demostrada. En ensayos siguientes, después de Salir se verifica por metadatos del proceso, sin pedir otro estado a Computer Use, y se inventariarán todas las instancias de la ruta QA antes de retirar su bundle.

La API de solo lectura de NSWorkspace confirmó después VoiceOver, aumentar contraste, reducir transparencia y reducir movimiento apagados. [Valores restaurados](../../.verification/native-accessibility/f3cdba51-e31c-46ab-b410-f8bee6fff52c/RESTORED_OS_METADATA.json). Esta lectura confirma el estado final, no los anuncios del lector durante el ensayo.

### Diagnóstico de activación posterior

El ensayo [11fa1ff7](../../.verification/voiceover-activation/11fa1ff7-b928-4c3d-8742-2c51ff23279e/CLOSURE.json), del 30 de octubre y limitado a tres minutos, contrastó el interruptor con `NSWorkspace.isVoiceOverEnabled=true`. El inventario de procesos localizó **VoiceOver Quickstart**, cuya ventana sí era accesible: «Bienvenido a VoiceOver», «Usar VoiceOver» y «Desactivar VoiceOver». Se eligió Usar sin marcar «No volver a mostrar este mensaje». Después se observó el proceso principal de VoiceOver; Computer Use siguió devolviendo timeout al seleccionarlo y su inventario lo marcaba como no ejecutándose.

La bienvenida explica un prerrequisito que faltaba en el recorrido; no demuestra que sea la única causa histórica ni un fallo de Gennie. La herramienta aún no permite observar los anuncios del lector. No se lanzó una nueva instancia del producto para sustituir esa evidencia. VoiceOver volvió a apagado y General quedó seleccionado; la API confirmó de nuevo las cuatro opciones apagadas. No se concedieron permisos ni se alteró la preferencia de bienvenida. Apple documenta que el [panel de subtítulos refleja la lectura de VoiceOver](https://support.apple.com/es-mx/guide/voiceover/unac078/mac); sin observarlo o escuchar el anuncio no se acredita el recorrido accesible.

## Entrega de modificadores al runner

La copia [c48afbb3](../../.verification/native-ui/c48afbb3-4884-4e52-8162-6d99838a2ba8/KEYBOARD_OBSERVATIONS.json) verificó entrada por coordenadas/`typeText` y foco visible del mensaje original al pulsar su cita. `shift+Return` y `Shift_L+Return` no insertaron línea ni enviaron; el texto renderizado quedó «Linea unoLinea dos». `shift+F10` tampoco abrió el menú. Con ese mismo mensaje enfocado, la tecla **Menu** sí abrió Responder; se mostró la previsualización y se canceló sin enviar. Esto distingue el foco funcional de la entrega del modificador, sin atribuir automáticamente un defecto al producto.

Los tests macOS de [desktop_accessibility_test.dart](../../frontend/test/desktop_accessibility_test.dart) y [message_reply_test.dart](../../frontend/test/message_reply_test.dart) conservan su prueba de eventos KeyDown/KeyUp explícitos. No sustituyen teclado físico ni los eventos no observados de Computer Use. La prueba nativa terminó producto/driver 0, dos mensajes/cita/sesiones intactos, cero trabajos/ejecuciones/modelos y ningún proceso residual de la ruta QA. Se integró el [guard de limpieza por ruta](../../.verification/native-residual-guard/20261029/REPORT.md) y no se pidió otro estado de la app después de Salir.

## Contraste aumentado y transparencia reducida

Seguimiento cuantitativo del 30 de octubre: el [ensayo 73da6dbe](../../.verification/high-contrast-measurement/73da6dbe-6ee9-4249-8e0d-c1640441da44/CLOSURE.json) terminó **sin verificar contraste**, con cero intentos PNG. macOS confirmó ambas preferencias activas, pero la ventana propia dejó de estar visible antes de medir; no se aisló la causa. Producto terminado por fallback (-15), driver 1; no se presenta como Salir por UI. Once tablas iguales, integridad/FK correctas y recursos propios retirados. Se restauraron Automático, ambos interruptores apagados y el tinte Liquid Glass a 0, comprobado al reabrir Aspecto; los interruptores habían cambiado también ese valor dependiente. El ensayo está cerrado, sin modelos. La evidencia cualitativa anterior conserva el alcance descrito debajo.

Ensayo independiente [2b999b74](../../.verification/native-display-preferences/2b999b74-c78e-4559-9515-26f9adcc2003/PLAN.json), máximo cinco minutos. Ambos ajustes inicialmente apagados se activaron mediante sus controles reales de macOS. Se observó el fondo opaco de la ventana QA y se recorrieron chat/compositor, lista y vacío de Encargos, ficha, propuesta y formulario de edición, en oscuro a 1100 × 720 y escala habitual. Texto y acciones quedaron visibles sin recortes observados en esas muestras. Se cerró la propuesta sin resolver y se canceló la edición sin guardar.

Ambos interruptores volvieron a apagado y Configuración del Sistema a General. Se pidió Salir en el bundle QA propio y se confirmó el PID terminado antes de recogerlo: producto/driver 0, todas las tablas iguales al baseline, integridad correcta, sin FK ni errores conocidos de motor, fixture/preferencias retirados. [Cierre](../../.verification/native-display-preferences/2b999b74-c78e-4559-9515-26f9adcc2003/CLOSURE.json). Cero turnos/guía, trabajos, ejecución de herramientas o cambios de Encargos.

Las capturas y árboles observados permanecen en la traza de Computer Use. Es evidencia cualitativa de esas superficies con dos preferencias reales activas; no mide contraste numérico, teclado físico, movimiento reducido, texto ampliado o toda la matriz nativa. No sustituye los renders/software y recorridos mínimos previos ni certifica cumplimiento integral de WCAG.


### Selección por ruta real del lector — 30 de octubre

El diagnóstico [707814be](../../.verification/voiceover-path-diagnostic/707814be-4eea-421e-9eed-e4adb5c8b7e0/CLOSURE.json) probó otra resolución documentada: NSWorkspace localizó /System/Library/CoreServices/VoiceOver.app; después de activar y continuar la bienvenida, ps confirmó su ejecutable principal PID 90351. Computer Use también devolvió timeoutReached al seleccionarlo por ruta completa y su inventario lo marcó isRunning=false mientras el PID existía. Por ello la selección por nombre/ID o ruta no proporciona observación de anuncios en este entorno. No se lanzó una nueva copia del producto.

Se restauraron VoiceOver apagado y General; API de solo lectura confirmó VoiceOver, contraste, transparencia y movimiento false. Sin nuevos permisos, cambios de bienvenida, modelos o guía. El [recorrido con observación humana](../../.verification/human-native-accessibility/20261030/PROTOCOL.md) se preparó sin iniciar y después quedó retirado por el usuario. No se reanuda ni se espera disponibilidad.

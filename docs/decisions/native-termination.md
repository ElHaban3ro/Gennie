# D-049 — retirar eventos de ventana antes de terminar Flutter

29 de octubre de 2026. Implementado y comprobado localmente. Continúa D-048 y el goal; sin cambios de UI, SQL, API Rust, configuración persistida, commit/push o despliegue.

## Causa y comportamiento

Salir cerraba correctamente el backend y devolvía código 0, pero el proceso registraba FlutterPlatformMessageCreateResponseHandle con motor inválido. La traza de la instancia propia d90bd0db-f28a-45a0-85bd-a00fe6195173 identificó un envío posterior al apagado por el canal window_manager, con callback, desde WindowManagerPlugin y un evento AppKit al ocultar/cerrar la ventana. No era una segunda petición de Rust ni evidencia de corrupción de SQLite.

El AppDelegate espera como antes la respuesta requestQuit de Dart, que cancela el stream y apaga Rust antes de aceptar. Solo si recibe true retira el delegado de la ventana principal antes de responder a AppKit. Así window_manager deja de emitir sus notificaciones cuando el motor ya se ha destruido. La solicitud permanece marcada como pendiente durante la terminación: una petición duplicada no vuelve a invocar un canal destruido.

False, ausencia, error o resultado inválido conservan el delegado y liberan el estado pendiente para permitir reintentar. Si desapareciera el AppDelegate mientras espera el callback, no autoriza el cierre por omisión. No se silencia el aviso, modifica Flutter o parchea la dependencia instalada. Cerrar para ocultar, navegación y apariencia conservan sus contratos durante el uso normal; el delegado solo se retira después del cierre confirmado del backend.

## Regresión e integración

python3 scripts/probes/native_termination.py compila el AppDelegate real con Cocoa/FlutterMacOS. Deriva una copia temporal retirando exclusivamente @main para usar el entrypoint del test; no sustituye la lógica de la clase. La ventana permanece oculta, no se inicia motor, Dart, Rust, cuenta o base de datos. Un NSApplication de prueba captura las respuestas sin terminar la aplicación real. Verifica aprobación, cuatro rechazos, conservación/retirada del delegado y duplicado sin nuevo uso del canal. La primera ejecución compilable falló en la retirada del delegado; corregida pasa. La búsqueda inicial del módulo de desarrollo se corrigió: el framework empaquetado no contiene sus headers. La regresión usa ahora fallos con exit(1), sin provocar Crash Reporter.

native_lifecycle_test.dart usa el canal real registrado por DesktopScreen con un backend ficticio: requestQuit permanece pendiente hasta acabar shutdown, un duplicado no lo repite y un fallo devuelve false con error visible y permite otro intento. El primer harness bloqueaba la respuesta en FakeAsync; se corrigió usando el mensajero de pruebas y runAsync, sin cambiar la aplicación para acomodar el test. Dos casos pasaron; la suite completa pasó 350 Flutter y 49 optativas omitidas, análisis sin incidencias. No se modificó Rust ni se repitió innecesariamente su suite.

La sonda optativa native_ui.py --require-clean-quit exige proceso ya salido con código 0, integridad/FK correctas y ausencia de los avisos conocidos de canal/motor en el log. Una terminación por timeout/señal no satisface esa comprobación. El mismo criterio falla sobre el artefacto nativo anterior aunque su exit code sea 0.

Tras el arreglo, la instancia 4c562763-9f71-47fa-86ed-0d11a86c25e2 repitió el recorrido con la misma traza observadora: trace_ready confirmó su carga y no hubo canales posteriores al apagado ni avisos. La instancia 33ce35b5-0dc3-4635-8faa-4ad1967dc146 lo repitió sin instrumentación, con el mismo resultado. Ambas salieron por Computer Use super+q, conservaron dos mensajes y planes en revisión 1, cero trabajos/ejecuciones, integridad/FK correctas y retiraron sus bases temporales. Se conservó el PID Debug previo del usuario.

## Evidencia y límites

Logs .verification/assistant-chat-real/termination-{before2,after,gate-before,widgets2,flutter,analyze,unit-final,release}.log. Manifiestos y logs nativos en .verification/native-ui/ con los identificadores anteriores; las acciones GUI están en el registro Computer Use. La traza se cargó únicamente en el proceso ficticio mediante una biblioteca externa; no se añadió instrumentación al producto ni se alteraron mensajes/callbacks para ocultar el fallo. Se conserva el código de diagnóstico bajo .verification/native-exit-diagnosis, fuera del producto.

Release compila con los cambios Swift y pasó firma ad hoc profunda/estricta, invariantes, diff y compilación del harness nativo. termination-{signature,invariants,diff}.log y termination-finish.json conservan el cierre; la biblioteca temporal de observación se retiró. No se abrió la DB personal, instaló el paquete o interrumpió la instancia Debug existente.

Estos ensayos no prueban una cuenta/modelo activo, audio físico, todos los plugins durante cierre, VoiceOver o cualquier aviso futuro de otra causa. El atajo físico de D-048 sigue pendiente por su entrega sintética sin Mayús. Continúan la evaluación empírica con guía y restantes verificaciones del plan; el goal permanece activo. Se preparó cd4264a3-9b10-4b9c-9545-4639313463a9 y pasaron línea base/protocolo con 23 registros, cero turnos o clasificaciones remotas y sin fallos automáticos. Su ejecución adicional requiere autorización; no se extiende el permiso agotado del piloto anterior.

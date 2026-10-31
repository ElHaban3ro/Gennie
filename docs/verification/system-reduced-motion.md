# Movimiento reducido: conservar la señal recibida por Flutter

30 de octubre de 2026. Arreglo mínimo en `frontend/lib/main.dart`, sin cambios de estilos, geometría, navegación, API, SQL o dependencias. La preferencia histórica de Presence permanece separada de la señal de accesibilidad del framework.

## Fallo y arreglo comprobados

GennieApp sustituía `MediaQuery.disableAnimations` por `snapshot.reducedMotion`, cuyo valor predeterminado es false. La regresión monta la app completa y usa el indicador real de Laya con un estado de carga ficticio: con petición de Flutter activada y preferencia local apagada, esperaba progreso estático 0,75 y recibía null, es decir, indeterminado. El fallo rojo está en [red.log](../../.verification/system-reduced-motion/20261030/red.log).

La combinación mediante OR mantiene la reducción si cualquiera de las señales la solicita. El test comprueba cambios de señal y de snapshot, retorno al indicador indeterminado al apagar ambas y ausencia de llamadas para guardar preferencias o cargar modelos. Pasa después del arreglo; no se reemplazó el consumidor por un getter aislado. [Parche y fuentes](../../.verification/system-reduced-motion/20261030/main.patch).

Esto demuestra la propagación dentro de Flutter. No prueba por sí solo que el embedder de macOS entregue su ajuste real: la [API de AppKit](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducemotion) identifica la preferencia del sistema y sus notificaciones, que requieren comprobación separada.

## Verificación y conservación visual

Análisis limpio; **398 pruebas Flutter correctas y 49 optativas omitidas**. Build release y firma local correctos, sin abrir, instalar o desplegar la aplicación. [Pasos y duración](../../.verification/system-reduced-motion/20261030/VERIFICATION.json), [identidad del release](../../.verification/system-reduced-motion/20261030/RELEASE.json). Ejecutable: `78ea953185f3bc5f13cfa03e0aec42c5c8c58614072ca0cc3788c7492fef2582`.

Comparación controlada con GennieApp real, plataforma macOS y fuentes locales: claro/oscuro, 1100 × 720/640 × 420 y texto normal/150 %. Las **ocho capturas del chat inactivo son idénticas en RGBA completas** con ambas solicitudes de movimiento apagadas. [Comparación](../../.verification/system-reduced-motion/20261030/VISUAL_COMPARISON.json). Son renders software, sin material nativo, interacción física o afirmación de cobertura de todos los estados. Se inspeccionaron también las muestras mínima oscura ampliada y normal clara.

Se cotejaron 96 fuentes/recursos del frontend: solo cambió main.dart; estilos, barra y demás archivos permanecen iguales. Archivos de origen antes/después, prueba de regresión y variante diagnóstica archivados; valores exportados de los logs de Xcode redactados. Rust y el puente no cambian, por lo que mantienen sus comprobaciones previas, sin repetir modelos o la suite Rust.

## Recorrido nativo ejecutado: defecto adicional observado

El Mac volvió a estar accesible. Se ejecutaron las dos copias QA selladas, con identidades y SQLite propias, cero modelos y máximo 180 segundos por copia. Se activó Reducir movimiento por Computer Use antes de cada apertura; el driver verificó true con NSWorkspace. Preferencia inicial false, restaurada tras cada observación y confirmada false mediante API al terminar. Ajustes regresó a General.

La copia anterior registró `QA_MOTION_DISABLED=false`, conforme al defecto original. La copia con OR también registró **false**, junto a `ENGINE_DISABLED=false ENGINE_REDUCE_MOTION=false`: el arreglo del consumidor Flutter es necesario, pero **no resuelve la transmisión de la preferencia real de macOS**. El guard del segundo driver termina en 1 por esa expectativa incumplida; no se acredita accesibilidad nativa correcta. No se forzó MediaQuery para cambiar el resultado.

Ambas ventanas renderizaron el chat/cita, sin recortes observados en esta muestra clara de 1100 × 720. Ambas salieron por Salir con código 0, sin errores de canal o procesos residuales. Citas/sesiones y dos mensajes conservados; cero trabajos/ejecuciones, SQLite íntegra, preferencia persistida false y fixtures retirados. No se modificó el release normal ni se abrió la base personal. La inspección visual está en el registro de Computer Use, no es un cálculo de contraste físico ni una prueba de animación activa.

[Resultado y restauración](../../.verification/system-reduced-motion/20261030/NATIVE_RESULT.json), [manifest anterior](../../.verification/system-reduced-motion/20261030/native-before/manifest.json), [manifest posterior](../../.verification/system-reduced-motion/20261030/native-after/manifest.json) y [señal posterior fallida](../../.verification/system-reduced-motion/20261030/native-after/MOTION_OBSERVATION.json). Los logs originales conservan las observaciones y el fallo. Las copias/proyecto temporales se retiraron tras cerrar; sus fuentes diagnósticas y hashes permanecen archivados, sin logging añadido al producto.

Siguiente cambio necesario: transmitir de forma explícita la lectura AppKit y sus notificaciones a Flutter, conservando OR con las señales heredada/local y retirando el observador al cerrar. No está implementado todavía; requiere regresión de señal inicial/dinámica, lifecycle y nueva prueba nativa tras compilar. La matriz accesible restante sigue pendiente; el goal vuelve a estar activo.

## Corrección AppKit y recorrido nativo completado

El seguimiento incorpora `SystemMotion` en Dart y un `FlutterEventChannel` de solo lectura en MainFlutterWindow. Lee `NSWorkspace.accessibilityDisplayShouldReduceMotion` antes del primer frame y observa `accessibilityDisplayOptionsDidChangeNotification`, sin temporizadores, escrituras o cambios de datos. GennieApp combina tres señales mediante OR: AppKit, framework y preferencia histórica. El observador se retira después de aprobar Salir; un rechazo lo conserva.

La regresión `system_motion_test.dart` entrega eventos por el canal real de Dart, monta GennieApp y comprueba su indicador de Laya, la preferencia local, una suscripción/cancelación y cero guardados/cargas de modelos. El primer intento del test omitía esperar el frame del snapshot: se corrigió la preparación; no fue un segundo defecto del producto. `native_termination.py` comprueba la retirada solo tras aprobación. **399 pruebas Flutter correctas/49 optativas omitidas, análisis/formato/build/firma y cierre nativo correctos**. Rust/helper conservan sus hashes anteriores. [Resultado](../../.verification/system-motion-appkit/20261030/RESULT.json), [release](../../.verification/system-motion-appkit/20261030/RELEASE.json), [fuentes verificadas](../../.verification/system-motion-appkit/20261030/SOURCES.json).

Una copia release exclusiva con observador pasivo debajo del MediaQuery del producto registró **true → false → true → false** al arrancar con Reducir movimiento activado, apagarlo, reactivarlo y restaurar false por Computer Use. No se forzaron señales, escala o tema. La segunda activación se observó al volver a poner la misma ventana en primer plano mediante Raise; no se reinició la app. Un error transitorio de ScreenCaptureKit al observar un cambio se resolvió leyendo el mismo handle: se confirmó off, sin repetir la acción. NSWorkspace confirmó true al arranque y false tras restaurar. [Secuencia](../../.verification/system-motion-appkit/20261030/native-after/MOTION_OBSERVATION.json), [manifest](../../.verification/system-motion-appkit/20261030/native-after/manifest.json).

Salir por menú terminó producto/driver 0, sin errores de canal ni residuales. Citas, dos mensajes/sesiones y revisiones ficticias intactos; cero trabajos/ejecuciones/modelos/guía. Ajustes regresó a General. Se retiraron fixtures, preferencias UUID y proyecto QA; el release habitual no se instaló o desplegó. La prueba acredita transmisión inicial y dinámica en este runner y condición; no sustituye VoiceOver, toda la matriz de formularios o una prueba de animación activa bajo carga nativa.

Ocho renders software de chat inactivo con señales apagadas siguen **idénticos en RGBA** a los anteriores: claro/oscuro, 640 × 420/1100 × 720, texto normal/150 %. [Comparación](../../.verification/system-motion-appkit/20261030/VISUAL_COMPARISON.json). Se conserva barra, geometría, fuentes y materiales. La muestra nativa clara de 1100 × 720 conserva chat/cita/compositor sin recorte observado; no es medición cuantitativa de contraste ni cobertura completa. El fallo nativo previo permanece archivado. Los siguientes huecos del goal siguen en la [matriz](native-matrix.md).

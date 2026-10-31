# D-053 — foco en el original al abrir una cita

29 de octubre de 2026. Cambio local de Flutter y del fixture nativo. Sin modificar Rust, FRB, SQL de producto, permisos, dependencias o el esquema/plan sellado de evaluación. Conserva estilo general y barra lateral.

## Hallazgo y arreglo

Abrir una cita localizaba/desplazaba el original, pero no le entregaba el foco. La regresión abre una cita por teclado entre 140 mensajes de dos sesiones y después intenta Responder con Mayús–F10: antes falló porque el menú no aparecía. El recorrido debía poder continuar sobre el original sin otro clic.

ChatTimeline posee nodos de foco únicamente para destinos de citas visitados. ChatMessage/MessageActions reciben el nodo opcional y reutilizan FocusableActionDetector y su indicador de foco ya existentes. Tras montar/localizar y desplazar el original se solicita foco allí. Un contador invalida el foco pendiente de una navegación anterior. Al retirar mensajes se liberan sus nodos después del frame que desmonta los hijos; al cerrar el lector se liberan los restantes. No se asigna un nodo propio a todos los mensajes ni se activa/reanuda la sesión original.

La referencia de interacción es [WCAG 2.2, explicación de orden del foco](https://www.w3.org/WAI/WCAG22/Understanding/focus-order.html): mantener significado y operación durante la navegación. Es orientación para este recorrido Flutter, no una certificación WCAG/VoiceOver de Gennie. No se introducen colores, geometrías o controles nuevos.

## Pruebas y capturas

Tres regresiones nuevas: cita por teclado y respuesta al original después de paginar; retirada del original enfocado con compositor aún editable; detalle de evidencia abierto por teclado y cerrado por Escape devolviendo foco al enlace, sin abrir el plugin. Las comprobaciones de cita preservan ausencia de envío y de selección/reactivación de otra sesión. La primera prueba falló antes del arreglo y pasa después.

Suite Flutter: 359 correctas, 49 optativas omitidas; 31 casos de citas, historial y evidencia correctos y análisis Dart limpio. La prueba de 140 mensajes usa backend de fixture, no demuestra comprensión de citas por un modelo real. El detalle de evidencia mantiene las comprobaciones de estado confirmado/fallido/desconocido y privacidad previas.

Las catorce capturas históricas no coincidieron: el análisis de píxeles localizó diferencias en la zona del pie del compositor. No se usan para atribuir un cambio a este arreglo ni se sustituyen sus registros. .verification/quote-focus/compare_current.py reconstruye solo la ausencia del cambio de foco en los tres archivos actuales, renderiza los mismos estados y restaura fuentes en finally. Esta comparación de la misma versión produjo catorce PNG idénticos byte a byte: dos citas con alto contraste y texto ampliado al mínimo, más doce estados de uso/detalle de evidencia claro/oscuro, normal/mínimo y texto 150 %. Archivos antes/después, hashes, fuentes y logs se conservan por separado; no se ejecutó esa mutación temporal junto a la compilación release. quote-original-focus-macos.png muestra además el estado nuevo con original enfocado tras paginar.

Logs quote-focus-{before,owner,flutter,analyze,visual,format,release} y .verification/quote-focus/{comparison,pixel-comparison,same-version-comparison}.json. La comparación histórica fallida y la comparación controlada correcta permanecen diferenciadas.

## Ejecución nativa

Release compilado y firmado localmente después de restaurar las fuentes. native_ui.py --require-clean-quit --quote-scenario prepara dos sesiones, dos mensajes y una cita de la sesión activa al original de la anterior, con modelos apagados y límite de 180 s. Se compara al cerrar la cita, la selección activa y los hilos de ambas sesiones. No hay cambios a una migración o base personal.

El fixture 602bcc95-413b-40d3-aa64-3af780b97158 (ejecutable de origen SHA-256 f685ebfb99cbcdc73b96efb7c655f9f43b1862eab75538a254049bdcc3a0a1f0) verificó en Computer Use: abrir la cita, indicador de foco en el original, tecla Menu que abre Responder, Down/Enter que selecciona ese original y cancelación de la referencia sin enviar. La sesión activa continuó siendo 2; hilos, cita, dos mensajes, revisiones 1/1 de encargos y cero trabajos/ejecuciones permanecieron iguales. Salir por UI devolvió 0, SQLite íntegro, cero FK inválidas o avisos conocidos de motor/canal; fixture y preferencias UUID retirados. La instancia Debug del usuario (49455) se conservó.

El intento sintético shift+F10 no mostró el menú; Menu sí. No se atribuye ese intento a un fallo de foco ni se declara probado Mayús físico. Las capturas/AX nativos están en la traza Computer Use y los manifiestos/observaciones en .verification/native-ui. El recorrido se observó a 1100 × 720, sin VoiceOver, envío a modelos o reproducción de voz. No acredita la matriz nativa completa, el mínimo o aislamiento semántico con proveedor activo.

quote-focus-finish.json consolida el cierre. El plan 187c5232-783e-4540-82d2-27012918e111 conserva fuentes/binarios relevantes y su solicitud de permiso; no se ejecuta sin respuesta humana. Continúan los demás requisitos del goal; sin commit/push, instalación o despliegue.


El [seguimiento nativo de historial amplio](../verification/native-large-history.md) localizó desde la última respuesta el primer mensaje entre 300 sesiones/600 mensajes, abrió Responder mediante Menu/Down/Return desde el foco entregado y canceló sin enviar. Lectura ro y hashes finales confirman sesión activa 300, cita/historial íntegros y cero hilos del proveedor. No es una prueba de Mayús físico/VoiceOver ni de todos los volúmenes; código/estilo sin cambios.

## Seguimiento: enlace del detalle dentro de MessageActions

La prueba aislada de ToolEvidence no cubría su ancestro real. El [recorrido 5ea0b802](../verification/native-tool-focus.md) reprodujo por teclado que Escape enfocaba el mensaje completo y no el enlace; el nuevo caso ChatMessage falla después del cierre. Mantener siempre la decoración de MessageActions evita desmontar el subárbol al cambiar su indicador. Borde/medidas/estilo conservados: doce PNG iguales, 385 Flutter/49 optativas, análisis/release/firma correctos. Nativo final acredita reapertura inmediata tras Escape y Listo, datos iguales y Salir 0; conserva la muestra intermedia con cierre fallido. Esta evidencia no sustituye modificadores físicos, VoiceOver o matriz completa.

# Foco del enlace de herramientas dentro del mensaje

29 de octubre de 2026. Preparación `.verification/native-tool-focus/5ea0b802-312c-46d0-b631-3882d970fe8b`. Conserva estilo/barra, medidas, colores y acciones existentes. Único cambio runtime: `frontend/lib/src/message_actions.dart`; no cambia Rust, FRB, SQL, permisos o dependencias. Dos mensajes breves y doce metadatos ficticios, sin ejecutar herramientas, modelos o guía; Presence/Assignments activos, otros módulos apagados.

## Fallo integrado y causa

El recorrido anterior del lector no acreditaba retorno del foco porque abrió por clic AX. Esta vez PID 16667 recorrió Tab hasta el enlace, cuyo borde y tooltip visibles acreditan foco; espacio abrió el detalle. Escape lo cerró, pero el foco quedó en MessageActions del mensaje completo y otro espacio no reabrió el detalle. Producto/script terminaron con 0 y datos intactos: el escenario de foco se declara fallido pese al cierre limpio.

La prueba aislada de ToolEvidence pasaba. Una regresión nueva con ChatMessage, su nodo MessageActions y el enlace real reproduce exactamente el fallo en linkFocus.hasPrimaryFocus después de Escape. No requiere ChatTimeline o actualizaciones de backend. Las hipótesis contrastadas fueron estructura condicional del borde, restauración del diálogo y reemplazo del historial; el caso mínimo excluye la tercera.

Container.build del SDK vigente añade su DecoratedBox solo con foregroundDecoration no nula. Al perder el foco para abrir el diálogo, MessageActions retiraba esa capa, desmontaba el enlace y destruía el nodo que Navigator debía recuperar. Mantener una BoxDecoration vacía conserva la estructura; el borde enfocado conserva su código y aspecto. Cambiar solo ese valor hace pasar la misma regresión y la reapertura inmediata por espacio, sin petición global o tardía de foco.

## Pruebas, aspecto y compilación

Regresión roja conservada, luego correcta; 32 casos afectados de evidencia/citas/historial y suite Flutter completa de 385 correctas/49 optativas omitidas. Análisis y formato correctos. Seis combinaciones ejecutadas como macOS: claro/oscuro, 1100 × 720, 640 × 420 y mínimo al 150 %. Doce PNG completos antes/después idénticos en RGBA, 24 muestras de contraste software con mínimo 5,498257:1 frente a 4,5:1. Se revisaron detalle oscuro mínimo ampliado y lector claro ampliado; no es certificación global/nativa. UI_CAPTURE_DIR aísla los nuevos artefactos sin sustituir capturas históricas.

scripts/build.sh compiló el release y verificó firma ad hoc local. No se instalaron/notarizaron paquetes. No se repiten suites Rust sin cambios de sus fuentes; el resultado anterior 415/28 conserva su ámbito. Fuentes antes/después, parche mínimo, drivers, resultados y capturas archivados.

## Repetición nativa y cierre

PID 25365, release corregido: Tab/espacio abre, Escape devuelve el borde al enlace y espacio reabre; Tab/espacio en Listo también devuelve foco y permite reapertura inmediata. **El cierre de esta muestra falla**: Computer Use pidió releer la app tras el clic en Salir; se envió entrada al padre cuando ps aún indicaba S+, y el harness terminó el hijo con -15. Se conserva run-after/manifest.json y NATIVE_AFTER_FAILED_CLOSE.json, con datos iguales, integridad y limpieza, sin declararlo cierre aprobado ni atribuir el fallo a Gennie.

PID 27812, otra base/copia del mismo release y fuentes, repitió los dos cierres y reaperturas por teclado. Quit del menú propio terminó el producto; ps confirmó Z+ antes de enviar entrada al padre. Producto/script 0, stop=already_exited, integridad/FK y motor/canal correctos. run-after-clean/manifest.json acredita sesión/historial/citas/metadatos/perfil/vínculos y cuatro encargos/revisiones/propuestas iguales, dos mensajes/doce registros, cero trabajos/ejecuciones/hilos. Los metadatos estaban precargados, no son llamadas producidas por esta prueba.

Las tres bases/copias/preferencias propias fueron retiradas y Debug 49455 permaneció vivo. Release final SHA-256 226b6d856307802a29c1bc43bade5f98682df2de868469f2a2fe962f622aa750. El sello de fuentes/binarios/rustc de la campaña de modelos preparada sigue coincidiendo. CLOSURE.json diferencia los fallos y la última muestra aprobada. Capturas/AX nativas están en Computer Use, sin PNG local exportado.

Se acredita este recorrido de teclas simples en ventana normal, sin afirmar modificadores físicos, VoiceOver, contraste nativo cuantitativo o la matriz completa de accesibilidad. Autorizaciones de modelos/sistema y los demás pendientes permanecen abiertos; goal activo, sin commit/push, instalación o despliegue ni cambios en datos personales.

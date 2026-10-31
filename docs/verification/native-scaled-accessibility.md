# Matriz nativa con texto ampliado y alto contraste

29 de octubre de 2026. Trabajo sobre una copia QA del frontend, fuente actual, ventana inicial 640 × 420, texto al 150 %, highContrast=true y disableAnimations=true dentro de MediaQuery. El runner macOS y backend release son reales. No se cambian preferencias del sistema ni se presenta esta variante como prueba de VoiceOver, entrega de modificadores o notificaciones de accesibilidad del OS. El estilo, medidas, materiales y barra del producto se conservan.

Preparación: .verification/native-accessibility-matrix/2eaf07c7-44c5-4bad-a3b0-aa01af85be24. Se copiaron 97 fuentes/manifiestos; la variante inicial modifica únicamente main.dart para el tamaño y las tres condiciones de Flutter. Fuente original y diferencias se conservan. La biblioteca Rust coincide por hash con el release de origen; no se inicia cuenta, modelo, navegador o envío externo. Cada ejecución nativa usa bundle y SQLite ficticios exclusivos, con límite de 180 segundos y retirada de preferencias/fixture propios. No se reinicia la instancia Debug del usuario.

## Hallazgo conservado

Run 55d6c0a9-4773-4440-ace3-b73530b46e87. La captura JPEG se midió como 640 × 420. Configuración permitió abrir Bot, Helper y Guía, opciones avanzadas y scroll manteniendo título/Cerrar/Guardar; se cerró sin guardar. Los tres perfiles y sus revisiones quedaron idénticos. Son observaciones de esas superficies y condiciones, no certificación de la matriz entera.

El perfil con nombre vacío llegó al backend: se mostró el rechazo, se añadió un mensaje system al chat y el error solo se hizo visible al desplazar manualmente el cuerpo. SQLite terminó con tres mensajes, perfil sin cambios y cero trabajos/ejecuciones. AX setValue no acreditó el vacío; super+a no seleccionó todo. BackSpace repetido sí dejó visible 0/200 antes de Guardar. No se infiere un fallo del teclado físico del producto a partir de esas limitaciones del input sintético.

La instancia venció el plazo y el harness la terminó con -15. Integridad/FK correctas y limpieza completa, pero **no pasa Salir limpio ni el gate de dos mensajes**. Se conserva BEFORE_OBSERVATION.json y el manifiesto original; no se relaja el gate para hacerla pasar. La propuesta de Encargos no se inspeccionó en ese recorrido.

## Arreglo y verificación local

AssistantProfileDialog rechaza nombres vacíos/blancos, multilínea o mayores de 200 escalares antes de DesktopState.perform. La validación Rust sigue vigente. El campo recibe foco y su grupo con el error se trae a la vista después del layout; movimiento reducido evita animar. Se mantiene el borrador y el camino existente para fallos reales del backend. No se transforma un rechazo local en un mensaje del chat.

La regresión de nombre vacío falló antes del arreglo porque había llamada al backend. Siete casos nuevos comprueban los rechazos, un nombre válido con carácter no BMP en el límite, foco y visibilidad en claro/oscuro, mínimo/150 %, alto contraste y movimiento reducido. Veintiséis pruebas afectadas correctas; suite Flutter 372 correctas/49 optativas omitidas y análisis sin incidencias. Rust no cambia por este arreglo.

Una comparación controlada en otra copia del mismo frontend mantuvo fuente, plataforma macOS, tipografías, imagen y fixtures iguales; solo cambió assistant_profile.dart. Seis capturas del perfil sin errores (claro/oscuro, 1100 × 720 y mínimo, escala normal/150 %) son idénticas en RGBA completas. CAPTURES.json y VISUAL_COMPARISON.json conservan resultados y PNG anteriores/posteriores. Esto prueba conservación visual de esas muestras software, no toda la composición física.

## Repetición nativa posterior y cierre

El producto habitual y la copia QA corregida compilaron en release y pasaron firma local. La copia conserva las mismas tres condiciones de Flutter y el tamaño; la biblioteca backend coincide con el producto por hash. Run ff2066d8-4d39-494c-8898-7b1cf9de81f4, ejecutable QA antes de la firma aislada 4ebdffb9b42725be8924cb3dbe1205065ef3aec5bc44570adf7b77e75bdf9f82.

Nombre vacío comprobado mediante BackSpace y 0/200 visible. Después de Guardar, el error quedó visible automáticamente, sin scroll manual, con foco en el campo según AX y caret. Escape cerró sin guardar y el chat conservó su contenido. Se abrió además una propuesta de Encargos, se desplazó hasta los últimos cambios con título y acciones fijos y se cerró con Escape sin aplicar. La captura JPEG volvió a medir 640 × 420.

Salir por el menú de la instancia QA se solicitó unos 57 segundos después de vincularla a Computer Use. El harness observó salida 0, SQLite íntegro/FK vacías, dos mensajes, perfil idéntico, arquitectos/revisiones idénticos, revisiones de Encargos 1/1 y cero trabajos/ejecuciones. Sin errores de motor/canal. Fixture y preferencias QA retirados; AFTER_OBSERVATION.json y el manifiesto conservan resultados. El primer fallo permanece separado.

Se archivaron fuentes antes/después y el parche exacto. Solo assistant_profile.dart difiere entre las fuentes habituales de UI anteriores y posteriores; main.dart del producto, estilos/barra y demás fuentes se conservan. La copia compilada y sus cachés de prueba se retiraron después del cierre; los archivos pequeños, capturas, logs, versiones y hashes siguen disponibles. La instancia Debug PID 49455 permaneció viva. No hubo nuevos modelos/guía y el sello de la campaña preparada 1e46eae9-edda-434e-9505-b4a69706daf0 sigue coincidiendo; su aprobación continúa pendiente.

Continúan VoiceOver, modificadores físicos, eventos reales del OS, alto contraste/texto ampliado nativos en oscuro y el resto de formularios/estados de la matriz. No se infieren valores WCAG nativos de una inspección visual, ni una matriz completa de estos recorridos. El goal sigue activo, sin commit/push, instalación o despliegue.

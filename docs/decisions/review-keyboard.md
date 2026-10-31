# D-056 — Escape y foco de los diálogos de revisión

29 de octubre de 2026. Fallo encontrado durante verificación visual nativa, sin cambiar estilos, barra lateral, geometría, materiales o textos de las acciones. Alcance: AppReviewDialog compartido por propuestas/ejecución de Encargos y retorno del foco del inspector de propuestas. Sin Rust, migraciones, FRB o dependencias nuevas.

## Reproducción y corrección

La copia QA ede2ee48-c06d-4591-b57d-f79926e3fa58 se redujo mediante Window → Move & Resize → Top Left. Captura nativa de 961 × 498: título, cuerpo y acciones de revisión visibles; Tab recorrió Cerrar/Descartar/Aplicar sin activar acciones. Escape no cerró el diálogo libre; Cerrar sí. Se conservaron cuatro encargos, cuatro revisiones y cuatro propuestas idénticos por hashes antes/después, sin guardados ni resolución. El fallo no depende de modelos o de datos personales.

AppReviewDialog tenía PopScope para busy, pero showDialog con barrierDismissible=false no ofrecía una cancelación por Escape. Ahora consume DismissIntent y cierra con resultado nulo únicamente si está libre. FocusScope mantiene la acción local disponible después de deshabilitar controles durante el guardado y de volver a habilitarlos tras error. PopScope conserva la protección durante IO; no se habilita el cierre por fondo ni se aplican/descartan datos por cancelar.

Las regresiones macOS también detectaron Scrollbar sin ScrollPosition: la vista no se asociaba al PrimaryScrollController en escritorio. SingleChildScrollView declara primary=true, compartiendo el controlador que ya esperaba la barra y evitando la aserción. No cambia sus medidas o estilo.

El inspector de propuestas deshabilita su fila durante consulta y refresco, por lo que Flutter perdía el foco del invocador. Conserva el nodo previo, espera el refresco y solicita foco tras reconstruir la fila habilitada. Revalida montaje, instancia, ausencia de otra operación y nodo todavía unido; una propuesta retirada durante el refresco no recibe foco ni causa una excepción. No se elimina la protección contra inspectores duplicados.

## Pruebas software

Tres regresiones en la ruta real de AssignmentProposals: abrir/cerrar por teclado y volver al invocador sin resolver; ignorar Escape mientras guarda y permitirlo después de un fallo sin segundo envío; original retirado al refrescar sin enfocar nodo desmontado. Las dos primeras reprodujeron el fallo antes. La primera versión corrigió Escape, pero dejó la aserción de Scrollbar; los logs intermedios se conservan. El retorno de foco también falló antes de corregir la consulta.

Suite Flutter: 362 correctas, 49 optativas omitidas. Quince pruebas del propietario correctas, con las seis composiciones visuales ejecutadas explícitamente como macOS; análisis y formato sin incidencias. Dieciocho PNG de petición/revisión/error, claro/oscuro a 1100 × 720 y 640 × 420, y texto al 150 % en el mínimo. Títulos y acciones permanecen fuera del scroll; contenido extenso se desplaza. Las dieciocho muestras de rótulos/error tienen mínimo 5,27361:1 frente al criterio 4,5:1; no certifican todo el producto.

El muestreador anterior tomaba el píxel sobre la caja de texto, que puede quedar fuera del botón compacto macOS al 150 %. Se conserva ese fallo en first-macos-measurement y se mide explícitamente dentro de Material y fuera del rectángulo de letras para los botones revisados; el umbral no baja y los demás consumidores mantienen su muestreo previo. CAPTURE_PREFIX evita reemplazar artefactos históricos. Montaje macos-matrix.png y resumen de contraste en .verification/review-keyboard; PNG/logs review-keyboard-* en .verification/assistant-chat-real.

## Evidencia nativa y límites

Antes del arreglo, 092a295a-87d9-4099-a0da-a94c5052abc8 comprobó cita→original→Menu/Down/Return→previsualización/cancelación, foco visible y perfil→Escape en el mismo tamaño compacto. Sesión 2, ambos hilos, cita y mensajes quedaron intactos, sin trabajos/ejecuciones. Ambos recorridos previos salieron por UI con código 0, integridad/FK correctas, sin avisos conocidos de motor/canal y recursos exclusivos retirados. La instancia Debug del usuario permaneció abierta.

Después de compilar release y verificar firma, f8814b43-8d81-446e-bcc7-9c8e9a6c078f repitió la revisión compacta: Escape cerró directamente, Tab llegó al invocador, Espacio abrió la revisión y Escape devolvió el indicador de foco a esa fila; otro Espacio la reabrió sin clic/Tab adicional. Se cerró otra vez sin aplicar/descartar. Hashes de las cuatro filas de cada tabla assignments/assignment_reviews/assignment_proposals idénticos antes/después; perfil, dos mensajes, revisiones min/max 1/1 y cero trabajos/ejecuciones intactos. Salir 0, motor sin los avisos conocidos, integridad/FK y retirada de paquete/DB/preferencias correctos. Fuente del ejecutable release 7486314cafc6ba82251d203dc08e617aced097a7ef4c04abee870adbdbb91139; la copia ad hoc tiene hash distinto por su identidad exclusiva. Bloqueo de Escape durante IO/fallo cubierto por widgets, sin atribuirlo a esta inspección nativa sin guardados.

Build, análisis, formato e invariantes correctos. review-keyboard-finish.json agrega comprobaciones y rutas sin sustituir los manifiestos de cada ensayo. No se recompila/repite Rust sin cambio de fuentes Rust; el cierre D-055 conserva sus 406 pruebas y Clippy. Sin instalación, commit/push o despliegue.

Las capturas/AX nativas están en la traza de Computer Use; no se exportaron PNG nativos. El mínimo exacto 640 × 420 no se alcanzó con el arrastre y no queda acreditado por 961 × 498 ni por capturas software. Persisten tema oscuro/alto contraste nativos, VoiceOver, teclado físico completo y otras superficies. No declarar la matriz o el goal completos. La prueba semántica de Encargos autorizada y cerrada se registra separada en [D-055](proposal-validation-feedback.md).

# Lector vacío, mensaje largo y detalle de herramientas en macOS

29 de octubre de 2026. Preparación y cierre: `.verification/native-reader-evidence/5fc131f3-5406-4af8-99e9-5cb9245a2220`. Dos copias propias del release habitual, 1100 × 720, identidades de bundle UUID y bases temporales independientes. Sin cambios de producto, estilo, navegación lateral o ajustes del sistema; solo Presence/Assignments activos. Conversación, guía, Plugins y ejecución apagados. La firma ad hoc se vuelve a verificar en cada copia; no se instala el paquete.

## Recorridos observados

1. PID 97787, lector con sesión activa y cero mensajes: Gennie visible en sidebar/cabecera, Inicio del historial, separador Nueva conversación y compositor. No se añadió mensaje ni se abrió una sesión del proveedor. Este es el estado de una sesión activa vacía, no el caso sin asistente seleccionado ni una afirmación sobre su texto vacío.
2. PID 99250, dos mensajes: solicitud de 42 caracteres y respuesta Markdown de 9.134, con sesenta apartados, tildes y Unicode. La carga mostró el final (apartados 58–60, marcador final y metadatos); desplazarse al comienzo mostró solicitud, marcador inicial y primeros apartados; volver abajo conservó el final. Un salto de treinta páginas produjo primero un fotograma sin texto. Un desplazamiento posterior de una página mostró el comienzo. Se conserva esta observación: no es un benchmark ni acredita ausencia de fotogramas transitorios.
3. Bajo la respuesta apareció «Usó: Herramienta ficticia · 4 fallos · 4 sin confirmar». Los doce registros están precargados, cuatro por estado finalizada/fallida/sin confirmar. Abrir el detalle mostró contador, servidor/función, modo de verificación de la primera llamada y avisos de acceso/permisos/resultado desconocido. El cuerpo se desplazó hasta function_12 con título y Listo visibles; no se ejecutó ninguna herramienta.
4. Escape cerró el detalle. Reabrir, Tab y espacio activaron Listo y devolvieron el lector al mismo final. Espacio inmediatamente después del primer cierre no lo reabrió; el árbol siguió atribuyendo foco al contenedor. Por tanto **no se acredita el retorno del foco al enlace invocador**. Esta instancia no prueba la acción Abrir plugin: el servidor ficticio no es un alias local.

Capturas y árboles están en la traza Computer Use; no se exportaron PNG locales ni se midió contraste nativo. No se activó VoiceOver ni preferencias de accesibilidad y no se usaron modelos/cuentas.

## Persistencia, cierre y límites

Ambos procesos salieron por Quit Gennie con código 0, antes de la recogida del padre. Los hashes de filas completas de sesiones, mensajes, citas, metadatos, perfil, vínculos, sesión activa, cuatro Encargos y cuatro revisiones/propuestas coinciden antes/después. Historial vacío conserva cero mensajes; el largo conserva dos y doce registros. Cero trabajos, intentos de ejecución o hilos de proveedor; revisiones de Encargos siguen en 1. Integridad/FK y canal Flutter limpios, bases/bundles/preferencias propias retirados y Debug 49455 conservado. `run-empty/manifest.json`, `run-long-tools/manifest.json`, `PREFLIGHT.json`, `CLOSURE.json` y archivo de fuentes conservan los controles.

El release mantiene SHA-256 `074eec394111fac8b38d4abd330acc21d7db4efb3cecc0479e92a257c73b0660`. No se repiten suites/build porque el runtime no cambió. El sello de fuentes de la campaña de modelos preparada sigue coincidiendo; las autorizaciones pendientes no se consumen. Esto amplía la evidencia nativa del lector y del detalle; no completa toda la matriz de accesibilidad, foco o rendimiento ni el goal. Sin commit/push, instalación o despliegue.

## Seguimiento: foco integrado corregido

La falta de acreditación anterior se investigó con apertura real por Tab/espacio. El [recorrido posterior](native-tool-focus.md) reprodujo pérdida de foco dentro de MessageActions, la corrigió conservando la capa de decoración y verificó reaperturas inmediatas tras Escape y Listo. Regresión, 385 Flutter/49 optativas, doce PNG iguales y release/firma correctos; última muestra nativa con datos intactos y Salir 0. La muestra intermedia con cierre -15 queda fallida y conservada. No se modifica retrospectivamente el resultado del primer lector ni se certifica toda la matriz.


## Formato real de las capturas de Computer Use

La preparación `.verification/native-contrast-feasibility/1c2352e7-a48d-466b-aa5a-2c8cd413dd19` abrió el release habitual en otra base ficticia de solo lectura, PID 29446. App.getScreenshot y App.getAXStateAndScreenshot devuelven aquí JPEG JFIF, 41.772/41.776 bytes, SOF0, 8 bits/tres componentes, 1100 × 720, submuestreo 2×2 de luminancia frente a cromas y perfil ICC presente. CAPTURE_FORMAT.json conserva los encabezados observados; no se exportaron capturas PNG. La API documentada no ofrece formato sin pérdidas; no se usa esa codificación para afirmar una medida exacta de contraste del material. Las cifras anteriores de widgets conservan su ámbito software.

Salir por menú, PID Z antes de recoger el padre y producto/driver 0. Timeline completo idéntico antes/después, integridad ok/FK vacías, cuatro intentos precargados, cero trabajos/mensajes/hilos/modelos/guía, fixture y preferencias UUID retirados. Producto, estilo/barra y preferencias del Mac sin cambios; Debug conservado. La ayuda local de screencapture confirma que existe captura PNG por ventana, pero no se ejecutó ni se preparó o autorizó esa vía alternativa: las interacciones de este trabajo permanecieron en Computer Use. Contraste nativo cuantitativo y accesibilidad real siguen pendientes.


## Captura PNG de ventana propia autorizada

La prueba efadaddb obtuvo un PNG de la ventana principal tras excluir un auxiliar 66 × 20 del mismo PID mediante tamaño/PID/ejecutable; los dos rechazos iniciales se conservaron. Conversión ICC/alfa y tres muestras localizadas, datos ficticios idénticos y Salir/script 0. [Informe y límites](../../.verification/native-lossless-capture/efadaddb-e872-47c1-8e35-10fa74e86ede/REPORT.md). La muestra detectó bajo contraste del estado vacío oscuro; [corrección software posterior](empty-state-contrast.md). El release corregido tuvo después confirmación visual cualitativa con Computer Use y cierre 0; no aporta medida PNG nueva, VoiceOver o preferencias reales.

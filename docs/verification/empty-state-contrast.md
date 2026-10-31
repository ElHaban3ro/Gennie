# Contraste de mensajes de estado vacío

Estado vigente, 30 de octubre: #555555 en claro y #a3a3a3 en oscuro. Cuatro muestras PNG nativas posteriores pasan 4,5:1; mínimo 4,73384:1. El seguimiento final conserva la regresión clara detectada después del primer arreglo oscuro y sus límites.

29 de octubre de 2026. La prueba PNG nativa autorizada detectó una muestra de 3,35468:1 en «No hay contenido disponible» del panel principal oscuro, anterior al arreglo. Fecha prevista 8,11571:1 y subtítulo de fila 5,53633:1 en esa misma captura. Alfa 255 en las regiones medidas y conversión ICC LG ULTRAGEAR→sRGB relativa; son muestras de una ventana/configuración, no una certificación global. [PNG, selección de ventana y cierre](../../.verification/native-lossless-capture/efadaddb-e872-47c1-8e35-10fa74e86ede/REPORT.md).

El texto de ContentEmpty y SidebarEmpty usaba el mismo gris #737373 en ambos temas. Un test renderizado sobre las superficies oscuras observadas reproduce 3,14982:1 en el estado vacío lateral, con fallo en escalas 100 % y 150 %; claro pasa. Este texto informa del estado, no es un control deshabilitado. Se usa como referencia el criterio 4,5:1 para texto normal de [W3C, WCAG 1.4.3](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum).

Corrección mínima: color compartido de mensajes vacíos #a3a3a3 en oscuro y el #737373 original en claro. Se conservan Inter 12/13, título 26, espacios, alineación, alturas, material, filas, navegación, iconos y estado de controles. No hay cambios de backend/API/SQL.

La regresión y los cuatro renders claro/oscuro a 640 × 420 y 100 %/150 % verifican legibilidad/ausencia de overflow. Contraste software sobre las superficies del fixture: oscuro 5,92086:1 lateral y 6,30594:1 central; claro 4,54285:1 en ambas. Fondos del fixture y método quedan explícitos en el test; no inferir contraste sobre cualquier escritorio o tema nativo.

Los primeros renders del fixture heredaban el subrayado amarillo de depuración por faltar Material; se conservaron como intermedios. Los finales usan Material transparente como la jerarquía normal y permanecen separados en captures-final. No fue un cambio de decoración del producto. La validación final y compilación se consignan en CLOSURE.json al terminar. El PNG nativo de referencia sigue siendo anterior al arreglo: la comprobación cualitativa del release corregido se registra al final.


Comprobación final: 73 pruebas de los consumidores afectados correctas, cuatro renders finales repetidos tras corregir el fixture Material, análisis sin incidencias y scripts/build.sh/firma 0. Release normal SHA-256 2342cb4a61b4af17f62f025fe0904f5f7ff797e0699f0cfd27ddfeed73d5462c; fuentes y 73 pruebas no equivalen a una suite completa nueva. El log de Xcode conserva comandos/resultados y retira los valores exportados del entorno.

El recorrido nativo corto con ese release/PID 73594 mostró el mensaje central y, tras introducir una búsqueda ficticia por teclado, «No se encontraron encargos»; gris corregido, marco/navegación/geometría conservados. AX setValue había cambiado el valor sin disparar el filtrado; no se afirmó búsqueda hasta observarla con entrada real del teclado sintético. Un SCStreamError transitorio se resolvió volviendo a vincular el mismo PID confirmado vivo, sin reinicio. Salir/driver 0, timeline íntegro idéntico, cero mensajes/trabajos/ejecuciones nuevos (cuatro precargadas), integridad/FK correctas y recursos retirados. Es confirmación visual cualitativa mediante JPEG de cua_repl, no nueva medida PNG ni VoiceOver. OBSERVATIONS.json y CLOSURE.json conservan alcance y límites.

## Seguimiento PNG real y ajuste claro — 30 de octubre

El escritorio desbloqueado permitió el [ensayo 934ed3d5](../../.verification/native-contrast-postfix/934ed3d5-b272-4532-b7bd-8c8ecf5a9eeb/CLOSURE.json), sobre el release a6e4f220. Dos capturas originales de una sola ventana/PID 1100 × 720, temas reales claro/oscuro, sin overrides QA ni datos personales. La petición explícita previa de tecnología PNG y la autorización continuada del goal sostienen el nuevo alcance; los presupuestos anteriores permanecen cerrados. Se restauró la selección inicial Automático y General. Salir/driver 0, once tablas iguales, dos mensajes, revisiones 1/1, cero trabajos/ejecuciones y recursos retirados.

Las muestras claras revelaron fondos sRGB #dbdbda lateral y #cececd central: #737373 alcanzaba solo 3,42236:1 y 3,01083:1. El fixture blanco anterior no reproducía ese vidrio. Oscuro pasó con 5,58985:1 y 5,97595:1. Se conservan originales, ICC, hashes y [mediciones previas](../../.verification/native-contrast-postfix/934ed3d5-b272-4532-b7bd-8c8ecf5a9eeb/MEASUREMENTS.json); no se reinterpretan como aprobado.

Se actualizó empty_state_contrast_test.dart con las superficies sRGB observadas: rojo claro en 100 %/150 %, oscuro correcto. El cambio mínimo es #555555 solo en los dos mensajes claros. Conserva Inter, tamaños, barra, geometría, marcos y vidrio. Cuatro renders y 76 pruebas afectadas pasaron, análisis/formato/invariantes/build/firma 0. La primera captura ampliada de assignments_test.dart falló por toImage antes del repintado; pumpAndSettle dentro de su rama CAPTURE_UI corrige únicamente el harness. El fallo y los 76 verdes quedan en sus logs; no se repite la suite Rust por una modificación de tinta Dart.

La primera repetición nativa [5243d770](../../.verification/native-contrast-light-fix/5243d770-8964-43ea-97ae-4b9cc7c69f5a/CLOSURE.json) se detuvo antes de capturar: el PID propio estaba vivo y accesible por AX, pero su lista de ventanas visibles estaba vacía. Permiso de captura disponible; cero PNG. Salir/driver 0 y limpieza intacta. No se amplió a escritorio, otra app o permisos. El ensayo nuevo [de574319](../../.verification/native-contrast-light-fix/de574319-157f-4b61-87ee-235bafad95ad/CLOSURE.json) contrastó metadatos propios de todas/visibles y ejerció Window→Bring All to Front. Su ventana ya era visible antes de esa acción; la causa exacta del fallo anterior sigue sin aislarse.

Release corregido d2a03118, backend/helper idénticos. Dos PNG de esa misma ventana y hash de framework, selección real de tema comprobada al capturar y render observado. [Mediciones finales](../../.verification/native-contrast-light-fix/de574319-157f-4b61-87ee-235bafad95ad/MEASUREMENTS.json):

| Mensaje | Claro | Oscuro |
|---|---:|---:|
| Vacío lateral | 5,38087:1 | 5,58985:1 |
| Vacío del lector | 4,73384:1 | 5,97595:1 |

PNG RGBA 1100 × 720, perfil LG ULTRAGEAR de 524 bytes conservado; conversión de análisis relativa a sRGB sin sobrescribir originales. Regiones identificadas de texto/fondo opaco, alfa 255, núcleos recurrentes de 132/177 píxeles, excluyendo bordes suavizados. Son cuatro muestras de dos mensajes sobre ese material, no contraste global, luminancia física, VoiceOver o certificación WCAG. Dos intentos consumidos, dos sobrantes cerrados. Automático y General restaurados, Salir/driver 0, once tablas iguales, integridad/FK correctas, cero modelos/efectos y recursos retirados.

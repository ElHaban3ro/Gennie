# Contraste de encabezados y etiquetas del editor

30 de octubre de 2026. AppDetailHeading mantiene label/subtítulo, tipografía, icono, título y acción. El editor de Encargos conserva campos, ayudas, distribución, foco y botones. Solo aumenta la opacidad de texto informativo en claro: 80 %. Oscuro mantiene 70 % en el encabezado, 65 % en etiquetas y 62 % en ayudas. Sin cambios de barra, material, geometría, API o datos.

## Regresión renderizada

heading_form_contrast_test.dart monta los componentes reales sobre las bases nativas sRGB observadas, con Inter y ventana 640 × 420, ambos temas y texto 100 %/150 %. Antes: cuatro casos claros fallan; encabezado 3,90402:1, etiquetas 3,47424:1 y ayudas 3,24372:1. Los cuatro oscuros pasan. Después: ocho casos nuevos y **96 pruebas afectadas correctas**, análisis/formato/build/firma correctos. No hay guardados o recargas en la prueba. La cobertura de dimensiones con sidebar completa conserva su prueba independiente; esta prueba aísla contraste.

El primer muestreador alineaba la etiqueta flotante al borde y leía fuera de imagen. Se conservan ese fallo y el intento de firma de API incorrecta; red-valid.log centra con Scrollable.ensureVisible antes de medir, comprueba el punto y reproduce los cuatro fallos claros de producto. No atribuir los errores de harness al producto.

## PNG nativo del release 366e0c1a

El [ensayo 659f5579](../../.verification/heading-form-contrast/659f5579-df05-4564-a51c-db6a1517bf25/CLOSURE.json) abrió una copia exclusiva, cuatro encargos ficticios y modelos/ejecución/guía apagados. Cuatro intentos PNG de una sola ventana propia 1100 × 720, con guard de PID/hash/framework/tema real. Las escenas efectivas fueron ficha clara, editor claro, ficha oscura y editor oscuro. El editor inicial ya muestra etiquetas y ayudas de hora/zona que heredan los estilos comprobados con preparación en widgets; el ajuste de escena se registró antes del segundo PNG, con idéntico presupuesto.

| Rol medido | Claro | Oscuro |
|---|---:|---:|
| Etiqueta de AppDetailHeading | 4,96161:1 | 6,64052:1 |
| Etiqueta flotante del editor | 4,96161:1 | 6,02714:1 |
| Ayuda del editor | 4,96161:1 | 5,55345:1 |

[Originales, regiones y mediciones](../../.verification/heading-form-contrast/659f5579-df05-4564-a51c-db6a1517bf25/MEASUREMENTS.json). RGBA/ICC LG ULTRAGEAR conservados; conversión relativa a sRGB solo para análisis. Fondos uniformes y alfa 255. Encabezado/ayuda tienen 70/71 píxeles de núcleo o más. La etiqueta flotante pequeña tiene solo cinco píxeles de cobertura completa: en claro coinciden con #525252, la tinta #333333 al 80 % sobre #cececd. Esos cinco puntos ocupan exactamente las mismas coordenadas de glifo en ambos temas. No usar un requisito arbitrario de 16 repeticiones para escoger en su lugar píxeles suavizados; se registran cuentas/coordenadas y se conserva el umbral 4,5:1. Son seis muestras, no contraste global del producto. El subtítulo opcional del encabezado tiene prueba de widget, no una muestra nativa separada en esta ficha sin subtítulo.

## Incidente de visibilidad y cierre separado

Configuración devolvió un timeout; se reinició solo el REPL de CUA y se recuperó la misma aplicación. La copia QA pasó a una selección Plugins no prevista, cuya causa no se aisló; su lista de ventanas visibles estaba vacía. Window→Bring All to Front hizo reaparecer el mismo PID/ventana en CoreGraphics. Se volvió a la misma ficha y editor, sin introducir texto, guardados o procesos nuevos, y se completaron las dos capturas oscuras. La inversión del orden oscuro conserva las mismas escenas y presupuesto.

Cancelar, Automático y General quedaron observados. Sin embargo, el ensayo alcanzó los 480 segundos antes de completar Salir: **producto -15, driver 1**, recogido por su watchdog. Se conserva fallido como prueba de cierre. Once tablas idénticas, integridad/FK correctas, dos mensajes, revisiones 1/1, cero trabajos/ejecuciones y paquete/DB/preferencias retirados; sin residuales. Las capturas tomadas durante su ventana validada conservan su alcance cuantitativo.

La [comprobación breve 2b839b3b](../../.verification/heading-form-contrast/2b839b3b-7277-43bb-b5e7-a0356d49e121/CLOSURE.json) usó el mismo release con otra copia exclusiva y límite 120 s: interfaz inicial lista y Salir, sin capturas o ajustes del sistema. **Producto/driver 0**, once tablas idénticas, integridad/FK/limpieza correctas, sin residuales o modelos. Prueba el cierre actual por separado; no convierte el ensayo vencido en aprobado. Backend/helper mantienen sus hashes y no se instaló el paquete.

## Siguiente hallazgo concreto

La frase existente «La tarea, sus fuentes y cuándo recibirla.» no usa los estilos anteriores: [mide 3,93688:1 en claro y 6,64052:1 en oscuro](../../.verification/heading-form-contrast/659f5579-df05-4564-a51c-db6a1517bf25/ADJACENT_MEASUREMENTS.json). Continúa pendiente junto con la revisión conjunta de resúmenes auxiliares; no extender los seis aprobados a todo el formulario ni añadir una función nueva para resolverlo.

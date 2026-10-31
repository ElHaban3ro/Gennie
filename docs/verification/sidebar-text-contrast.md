# Legibilidad de textos auxiliares de la columna

30 de octubre de 2026. Revisión del contraste exigido por el plan, con el estilo y la barra vertical conservados. Solo cambia la tinta de cabecera, indicación de búsqueda y subtítulo normal claro; no cambian fuentes, tamaños, iconos, disposición, bordes, fondos o geometría. La fecha con color propio conserva su estilo.

## Evidencia antes y corrección

El [ensayo b55ee9a7](../../.verification/native-sidebar-labels/b55ee9a7-dfbb-46c2-a90c-4754e008df77/CLOSURE.json) usó el release d2a03118, cuatro encargos ficticios y una ventana propia de 1100 × 720, temas reales y dos PNG. Medidas nativas sRGB: título 9,11922:1 claro / 10,97219:1 oscuro; subtítulo 3,47209:1 claro / 5,32269:1 oscuro; cabecera 3,42236:1 claro / 2,97373:1 oscuro; placeholder 2,63033:1 claro / 4,01952:1 oscuro. Estos textos son informativos, no controles deshabilitados.

sidebar_text_contrast_test.dart monta los componentes reales y deja que AppSidebar componga su tinte sobre la base nativa observada. Cuatro casos rojos, claro/oscuro a 100 %/150 %. La cabecera y el placeholder pasan a usar el gris informativo existente: #555555 claro / #a3a3a3 oscuro. El subtítulo predeterminado conserva la opacidad del 62 % en oscuro y usa 80 % en claro, para adaptarse también al fondo seleccionado. Se respetan overrides, incluidos los de fecha.

Cuatro casos verdes y cuatro capturas software, 86 pruebas afectadas correctas, análisis/formato/invariantes/diff/build/firma 0. Veinte mediciones software, mínimo 4,62708:1. La suite completa anterior 406/49 y las pruebas Rust conservan su alcance; no se presentan como repetidas. Backend/helper mantienen sus hashes.

## Comprobación nativa posterior

El [ensayo c8caf918](../../.verification/native-sidebar-labels/c8caf918-700b-48aa-b708-d319d26fbf65/CLOSURE.json), release 089662a1, seleccionó la primera fila solo para consulta. Dos PNG de su mismo PID/ventana, con hash de ejecutable/framework y tema real comprobados. Ninguna edición, aplicación de propuesta, activación o modelo.

| Texto medido | Claro | Oscuro |
|---|---:|---:|
| Cabecera de columna | 5,38087:1 | 5,58985:1 |
| Indicación de búsqueda | 5,38087:1 | 5,58985:1 |
| Subtítulo sin seleccionar | 5,38087:1 | 5,32269:1 |
| Subtítulo seleccionado | 4,96161:1 | 4,69934:1 |
| Título seleccionado | 8,02267:1 | 9,23214:1 |

[Mediciones y regiones originales](../../.verification/native-sidebar-labels/c8caf918-700b-48aa-b708-d319d26fbf65/MEASUREMENTS.json): PNG RGBA 1100 × 720, ICC LG ULTRAGEAR de 524 bytes conservado, conversión de análisis relativa a sRGB. Alfa 255 en las regiones; núcleos recurrentes de glifos y fondos uniformes adyacentes. Son diez muestras concretas, no una certificación global.

Ambos ensayos cerraron con producto/driver 0, once tablas idénticas, dos mensajes, revisiones 1/1, cero trabajos/ejecuciones y recursos/preferencias QA retirados. Automático y General restaurados. El primer ensayo encontró tiempos de espera de Configuración y un reset de CUA; se recuperó la misma aplicación sin reiniciarla ni usar otra tecnología de UI, y la restauración quedó confirmada antes de Salir. Dos intentos PNG consumidos y dos sobrantes cerrados en cada alcance.

## Hallazgo adyacente aún pendiente

Las mismas imágenes permiten medir los dos rótulos de entrega de la ficha: «Cuándo recibir la información» y zona horaria. [Mediciones separadas](../../.verification/native-sidebar-labels/c8caf918-700b-48aa-b708-d319d26fbf65/DETAIL_MEASUREMENTS.json): claro 3,93688:1, oscuro 6,64052:1. Conservan pendiente el ajuste claro y su regresión/verificación. No se presentan los diez resultados laterales aprobados como prueba de esos otros textos.

Este hallazgo adyacente se corrigió después en su propio [ensayo de entrega](assignment-delivery-contrast.md): cuatro muestras nativas pasan; las medidas fallidas anteriores se conservan.

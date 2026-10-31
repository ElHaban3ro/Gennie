# Línea de foco del editor de Encargos

30 de octubre de 2026. El foco del campo activo usa la misma línea inferior de un píxel; su tinta pasa de 50 % a 65 % solo en claro. Oscuro conserva 50 %. No cambian campos, tipografía, disposición, fondos, borde inactivo o barra lateral.

## Evidencia renderizada

[Preparación y registros](../../.verification/assignment-focus-contrast/961c821e-2538-4b36-b25a-012ab3c311ce/PLAN.json). Cuatro casos a 640 × 420, Inter, claro/oscuro y texto 100 %/150 %, con el editor real. Enfocan el campo, comprueban foco y línea pintada, y calculan el contraste de su tinta declarada contra la superficie adyacente rasterizada. Exigen 3:1 y cero guardados. Dos casos claros fallaban con **2,491889:1**; oscuro ya pasaba. Tras el cambio pasan los cuatro y las **39 pruebas afectadas**.

La primera versión del medidor exigía 3:1 a un píxel suavizado de una línea situada entre coordenadas fraccionarias; fallaba también en oscuro ampliado. Se conserva como fallo del harness. La regresión válida usa la tinta declarada sobre el fondo medido, además de comprobar la línea realmente pintada y el foco. Es el método indicado por [W3C para contraste no textual](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html): el antialiasing no reemplaza el color declarado. No se aumenta el grosor ni se cambia el diseño para acomodar el medidor.

## Suite, release y ventana nativa

La suite completa terminó con **434 correctas/49 optativas omitidas**; análisis, build oficial y firma local correctos. [Resultado y sellos](../../.verification/assignment-focus-contrast/961c821e-2538-4b36-b25a-012ab3c311ce/RESULT.json). Release `4ae14b8eb22e74f7aebaee77cbeb3042219681f29eb028840500ca832b2fc5db`, backend/helper idénticos, sin instalar/desplegar. Las fuentes Rust de producción y las 55 migraciones siguen iguales a su puerta anterior; solo el test público de opcionales añadido conserva su comprobación dirigida independiente.

Una copia QA propia del release, claro real y 1100 × 720, abre el editor y enfoca Título. q aparece y BackSpace lo retira antes de capturar; el campo vacío conserva foco/caret y línea. Un error transitorio SCStream -3811 se conserva; se comprobó el mismo PID vivo/ventana visible y se reobservó, sin reiniciar. Un único PNG permitido, después Cancelar y Salir. Producto/driver 0, once tablas idénticas, dos mensajes/revisiones 1/1, cero trabajos/ejecuciones, integridad/FK correctas y fixture/preferencias retirados, ningún residual o modelo.

[Medición](../../.verification/assignment-focus-contrast/961c821e-2538-4b36-b25a-012ab3c311ce/MEASUREMENTS.json): línea #686868 sobre #cececd, **3,53827:1**, 300 píxeles uniformes en una única fila. Original RGBA/ICC preservado y análisis relativo sRGB, alfa 255; regiones y hashes registrados. El grosor observado sigue siendo un píxel. La captura acredita esa línea, sin certificar contraste global o toda la matriz.

## Límites y continuación

Oscuro conserva su tinta y pasa regresión; no se toma otra muestra nativa oscura. VoiceOver permanece fuera del alcance. La auditoría estática localizó también focusedBorder al 50 % y el icono Limpiar al 55 % en SidebarSearch: se midieron y corrigieron después en el [seguimiento conjunto](sidebar-control-contrast.md), con dos muestras nativas a 3,79002:1 y geometría conservada. Esos controles no se modificaron en este ajuste anterior.

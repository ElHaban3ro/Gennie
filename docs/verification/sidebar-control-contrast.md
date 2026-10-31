# Foco y Limpiar del buscador lateral

30 de octubre de 2026. SidebarSearch conserva disposición, fuentes, dimensiones, iconos, radio de 8 px, grosor de borde y material. Solo el foco y el icono Limpiar usan tinta al 65 % en claro; oscuro conserva 50 %/55 % respectivamente. La lupa, el borde inactivo y la barra vertical se conservan.

## Regresión y comparación visual

[Alcance y artefactos](../../.verification/sidebar-control-contrast/ae60d0c3-71a2-4ba3-b4a8-ab7ffbc95eb1/PLAN.json). Cuatro casos montan AppSidebar/SidebarSearch reales a 640 × 420, ancho 216, Inter y texto 100 %/150 %, claro/oscuro, sobre la base observada por PNG nativo. Comprueban foco efectivo, borde pintado, límites del control/icono y Limpiar con un único callback y campo vacío. Miden tinta declarada frente al fondo adyacente rasterizado y exigen 3:1 para esos indicadores no textuales.

Dos casos claros fallaban: foco **2,59413:1**, Limpiar **2,90729:1**; oscuro ya pasaba. Tras el ajuste: **3,69237:1** en ambos controles claros, oscuro 3,92884:1/4,43099:1. Las **103 pruebas afectadas** pasaron, incluyendo capturas opcionales de los consumidores.

[Comparación RGBA](../../.verification/sidebar-control-contrast/ae60d0c3-71a2-4ba3-b4a8-ab7ffbc95eb1/RENDER_COMPARISON.json): ambos renders oscuros idénticos; los claros cambian 657 píxeles cada uno, todos dentro del contorno de foco o del icono Limpiar. Cajas idénticas y cero cambios fuera. La primera máscara rectangular omitía cinco píxeles de las esquinas redondeadas; se conserva el diagnóstico y se usa distancia al contorno existente de radio 8, sin excluir toda la caja del buscador.

## Salidas de captura

La primera selección activó CAPTURE_UI para todos los consumidores: cuatro capturadores históricos ignoraban UI_CAPTURE_DIR y regeneraron **49 artefactos mutables** de widgets. No se registraron hashes previos; esos archivos no se atribuyen a una versión histórica sin cotejar su sello. [Inventario e incidente](../../.verification/sidebar-control-contrast/ae60d0c3-71a2-4ba3-b4a8-ab7ffbc95eb1/REGENERATED_LEGACY_ARTIFACTS.json). Las fuentes y capturas nativas con directorio propio no fueron esas salidas.

captureOutput centraliza el destino solicitado en las pruebas de chats, Encargos, Plugins y MCP. Los valores históricos por defecto se conservan; cada ensayo debe indicar UI_CAPTURE_DIR exclusivo. La repetición de las 103 pruebas ejercitó los capturadores reales y escribió su propio directorio, conservando los 49 hashes del inventario. [Guard de salida](../../.verification/sidebar-control-contrast/ae60d0c3-71a2-4ba3-b4a8-ab7ffbc95eb1/CAPTURE_OUTPUT_GUARD.json). No cambia código de producto o parámetros de comparación por esta corrección de salida.

## Suite y ventana nativa

Suite completa: **438 correctas/49 optativas omitidas**. Análisis, build oficial y firma local 0; [resultado, fuentes y cierre](../../.verification/sidebar-control-contrast/ae60d0c3-71a2-4ba3-b4a8-ab7ffbc95eb1/RESULT.json). Release `bc5729349d5d1ac008338e31a3bee8b3165b8e3e739ea89b28227de92f78b430`, backend/helper idénticos, sin instalar/desplegar.

Copia QA propia, claro real, 1100 × 720: cuatro encargos ficticios, escribir 4 deja una fila y muestra foco/Limpiar; un único PNG de la ventana exacta. La primera activación AX de Limpiar devolvió un aviso transitorio; después se observó un espacio adicional de origen no aislado. El PID siguió vivo, se reobservó y un clic en la posición visible limpió el campo y recuperó las cuatro filas. Se conserva este límite del recorrido por AX, sin presentarlo como acción nativa por nombre acreditada. Salir/driver 0, once tablas idénticas, dos mensajes/revisiones 1/1, cero trabajos/ejecuciones, integridad/FK y limpieza correctas. Ningún proceso residual, modelo o cambio de preferencias del sistema.

[Dos muestras nativas](../../.verification/sidebar-control-contrast/ae60d0c3-71a2-4ba3-b4a8-ab7ffbc95eb1/MEASUREMENTS.json): foco e icono, ambas **3,79002:1**; núcleo #6c6c6c sobre #dbdbda, 120/48 píxeles de cobertura, alfa 255. Original RGBA/ICC preservado y análisis relativo sRGB; regiones/perfil/hash registrados. La línea mantiene un píxel. Se supera 3:1 en esas regiones del material real, sin certificar contraste global.

Oscuro conserva sus tintas y los renders son idénticos; no se toma otra muestra nativa oscura. Los seis PNG con sello propio de los últimos textos/foco conservan sus hashes primarios tras el incidente de salidas mutables. VoiceOver permanece fuera del alcance por instrucción del usuario.

La revalidación final de Rust pasó 436/28, formato y Clippy del workspace/all-targets; las fuentes permanecieron congeladas. No se repiten campañas de modelos por estos cambios de tinta/capturadores.

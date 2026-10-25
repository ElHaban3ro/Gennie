# Revisión de la segunda fila

Se trazaron cuatro SVG desde los recortes originales corregidos de 362 × 362. Los documentos finales tienen dimensiones 512 × 512, viewBox de 362 × 362 y fondo transparente. No contienen imágenes raster incrustadas. Las formas son trazados Bézier y círculos/elipses para ojos y pico.

La primera comparación está en `second-*-comparison-v1.png`. Tras inspeccionarla se corrigieron los tonos del zorro (el primer naranja y la cola eran demasiado luminosos), el azul de verdes y grises, la crema inferior del búho, los centros de ojos medidos en los originales y los extremos de pies que quedaban entre 0.7 y 1.5 px por encima de la referencia. Las parejas finales están en `second-*-comparison-v2.png`.

La señal geométrica aproximada de `second-metrics-v2.json` da intersección/unión de silueta de 0.9767 para zorro, 0.9764 para búho, 0.9839 para pingüino y 0.9805 para rana. Se utiliza un umbral respecto al fondo para medir el original, por lo que la cifra incluye incertidumbre por bordes suaves; no demuestra equivalencia visual.

Los SVG conservan posiciones, tamaño general y distribución de rasgos, pero no son reproducciones píxel a píxel. Permanecen diferencias locales de contorno de aproximadamente 1–3 px, con alguna diferencia mayor en las curvas del extremo inferior del búho y la cola del zorro. No se reconstruyeron el grano fino ni todas las variaciones de color de la ilustración raster: los rellenos vectoriales se ven más lisos.

No se ejecutaron Cargo, Flutter, servidores ni aplicación. La comprobación fue XML/renderizado local mediante librsvg y revisión visual de las ocho parejas. No se hicieron commits.

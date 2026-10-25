# Revisión de gato, perro, conejo y oso

Se abrieron los recortes originales corregidos y se trazaron curvas Bézier con viewBox 362 × 362. Las cuatro salidas tienen tamaño nominal 512 × 512 y fondo transparente, sin raster incrustado.

Iteración 1: primer trazado. Durante esta iteración se corrigieron externamente los orígenes de los recortes; esa comparación no representa el resultado final.

Iteración 2: reapertura de las cuatro referencias corregidas y traslado a sus coordenadas. Se rehicieron los pies completos. La revisión detectó un segmento mal formado en el cuerpo del conejo y pies derechos demasiado pequeños en el gato.

Iteración 3: cuerpo del conejo reparado, pies de gato y oso corregidos y centros de ojos puestos según medición de las referencias.

Iteración final: colores de orejas del gato, orejas del perro, interior de orejas del conejo y cuerpo del oso ajustados con píxeles fuente. La comparación visual final está en first-comparison-final.png; métricas en first-metrics-final.json.

Diferencias restantes: la fuente tiene una textura suave de variación de color que no se reproduce píxel a píxel; los SVG conservan gradientes muy discretos. Los contornos son aproximaciones manuales con desviaciones locales pequeñas, especialmente en la unión de las orejas y pies. El perro queda de 1 a 2 px dentro del bbox fuente y el oso 1 px dentro por arriba/derecha. La métrica IoU usa una estimación del fondo, por lo que no constituye igualdad exacta ni mide fidelidad de color.

Validación: los cuatro SVG se analizaron como XML y se renderizaron con librsvg a 362 y 512 px. Se comprobó tamaño 512 × 512, transparencia de esquina y ausencia de elementos image.

# Mascotas minimalistas — revisión 3

Doce SVGs independientes, editables, con fondo transparente. Cada archivo tiene tamaño nominal 512 × 512 y viewBox 0 0 362 362. Los identificadores internos llevan el nombre del animal para evitar colisiones al insertar varias mascotas en un documento.

## Referencia y revisión

Esta revisión parte de la imagen original generada, no de las versiones 1 y 2. Tres subagentes GPT-6.1 Sol con esfuerzo Medium trabajaron por grupos de cuatro mascotas. Cada uno abrió sus referencias, trazó las formas, renderizó a la misma escala, comparó y realizó varias pasadas de ajuste. Se revisaron siluetas, patas/aletas, orejas/branquias, ojos, colores y manchas. La aleta izquierda de la foca conserva su posición pegada al cuerpo de la referencia. La punta clara de la cola del zorro llega al contorno exterior; la zona naranja está recortada para evitar bordes naranjas alrededor de la punta.

comparison.png muestra cada original junto a su SVG a la misma escala. preview.png y preview.svg muestran solo la colección vectorial. Los recortes de referencia y registros de iteración están disponibles en las carpetas reference y review del directorio de trabajo; el ZIP de entrega contiene los SVGs, las vistas de conjunto y este documento.

Hay pequeñas diferencias de curvas y de textura. El raster original tiene grano y variaciones que los rellenos vectoriales suaves no reproducen exactamente. La comparación de máscaras registra coincidencia de silueta aproximada entre 97,6 y 98,9 por ciento; es una medición geométrica orientativa, no un porcentaje de identidad visual. La posición de los ojos se contrastó con centros medidos en la imagen fuente.

## Archivos

cat.svg — gato; dog.svg — perro; rabbit.svg — conejo; bear.svg — oso; fox.svg — zorro; owl.svg — búho; penguin.svg — pingüino; frog.svg — rana; axolotl.svg — ajolote; turtle.svg — tortuga; hamster.svg — hámster; seal.svg — foca.

## Comprobaciones

XML válido, identificadores únicos, ausencia de imágenes rasterizadas o recursos externos en las mascotas, transparencia y renderizado de los doce archivos a 64 y 512 píxeles. Revisión visual de las comparaciones. Esta biblioteca es una entrega independiente; no está integrada en la aplicación Gennie.

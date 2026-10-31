# Contraste del detalle de entrega

30 de octubre de 2026. Las mismas capturas del [seguimiento de columna](sidebar-text-contrast.md) mostraron los rótulos «Cuándo recibir la información» y zona horaria a 3,93688:1 en claro y 6,64052:1 en oscuro. No son controles deshabilitados. Se conservaron esas mediciones fallidas.

AssignmentProfile usa ahora tinta al 80 % en claro y mantiene el 70 % existente en oscuro para esos dos textos. Se conservan fuentes, tamaños 12/11, grupo, bordes, espacios y disposición; no cambia el editor, la barra o el material general.

assignment_delivery_contrast_test.dart monta la ficha real sobre las bases sRGB nativas. Dos regresiones claras rojas al 100 %/150 %, oscuro correcto; después, cuatro verdes con ambos textos dentro del viewport y sin recorte al mínimo 640 × 420. Las 35 pruebas afectadas, análisis/formato/invariantes/diff y build/firma pasaron. No se presenta como nueva suite Flutter completa ni se repite Rust por un cambio de tinta Dart.

El [ensayo 60f59230](../../.verification/assignment-delivery-contrast/60f59230-2ae9-4cf3-b9d5-f833e4e77fb0/CLOSURE.json), release 47d2fbba, consultó la primera de cuatro filas ficticias. Dos PNG de la misma ventana propia de 1100 × 720, PID/ejecutable/framework y tema real comprobados antes de cada captura. Cero modelos, edición, guardado, aplicación de propuestas o activaciones.

[Mediciones finales y regiones](../../.verification/assignment-delivery-contrast/60f59230-2ae9-4cf3-b9d5-f833e4e77fb0/MEASUREMENTS.json): ambos rótulos **4,96161:1 en claro y 6,92687:1 en oscuro**. Originales RGBA/ICC LG ULTRAGEAR conservados; conversión relativa a sRGB solo para análisis, alfa 255 y núcleos recurrentes de glifos sobre fondo uniforme. Cuatro muestras concretas, no contraste global de la ficha o del producto.

Automático y General restaurados. Producto/driver 0, once tablas iguales, dos mensajes, revisiones 1/1, cero trabajos/ejecuciones, integridad/FK correctas y paquete/DB/preferencias QA retirados; sin procesos residuales. Backend/helper mantienen sus hashes. Dos intentos PNG consumidos y dos sobrantes cerrados; no se reutilizan presupuestos de ensayos anteriores.

La revisión restante de rótulos auxiliares de otros encabezados/formularios conserva su alcance propio. Este aprobado no se extiende a opacidades de etiquetas del editor ni a controles no medidos.

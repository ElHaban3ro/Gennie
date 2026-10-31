# Textos auxiliares de Encargos — 30 de octubre de 2026

La introducción, el vacío de aplicaciones y los resúmenes plegados del editor usan tinta al 80 % en claro y conservan el 70 % en oscuro. La ficha comparte ese criterio para URL y resúmenes; números y viñetas usan 80 % claro y conservan 65 % oscuro. No cambian fuentes, tamaños, disposición, controles, iconos, bordes, material o barra lateral.

## Regresión y compilación

[Fuentes, planes y registros](../../.verification/assignment-secondary-contrast/3b9fac67-8c62-4335-a60c-dc2af17270a4/RESULT.json). Ocho regresiones de contraste: editor/ficha, claro/oscuro y texto 100 %/150 % a 640 × 420. Cuatro casos claros fallaban con la tinta anterior; cuatro oscuros ya pasaban. Tras el ajuste: 58 afectados y suite completa **430 correctas/49 optativas omitidas**, análisis y build/firma correctos. El medidor resuelve también EditableText, que es el resultado del finder para SelectableText; el primer ensayo del harness sin esa rama se conserva separado de la regresión válida.

Release normal: `9a3a93a3a78aeb3b41f00de6bda662f0388afaad9ba4b4eae0938a47c4dcbb55`. Backend/helper no cambian; no se instala o despliega. La [revalidación narrativa](architect-communication.md#revalidación-del-alcance-actual--30-de-octubre-de-2026) conserva los límites helper/guía → bot → usuario; no requiere otra campaña de modelos por este ajuste Dart.

## Ventana nativa

[Ensayo 401435ac](../../.verification/assignment-secondary-contrast/401435ac-06e9-43e7-999a-45c2b1749155/PLAN.json): mismo release, copia con identidad exclusiva y base ficticia, claro real, 1100 × 720. Dos PNG de una única ventana propia: introducción; aplicaciones y resúmenes plegados tras scroll. Se cancela el editor sin guardar y se usa Salir. Producto/driver **0**, once tablas idénticas, dos mensajes, revisiones 1/1, cero trabajos/ejecuciones, integridad/FK correctas, fixture/preferencias retirados y ningún proceso residual. No se cambian preferencias del sistema ni se inician modelos.

[Seis mediciones](../../.verification/assignment-secondary-contrast/401435ac-06e9-43e7-999a-45c2b1749155/MEASUREMENTS.json): introducción, ninguna aplicación, método, contexto, preparación y estado, todas **4,96161:1**. Originales RGBA con ICC preservados; análisis relativo en sRGB, fondo uniforme #cececd, núcleo #525252 con al menos 100 píxeles de cobertura y alfa 255. Cada muestra conserva regiones, perfil y hash. Se supera 4,5:1 en esas regiones; no certifica contraste global.

El primer intento 3b9fac67 terminó por plazo con ventana propia no disponible, producto -15/driver 1 y cero capturas. El diagnóstico a5426edb observó la ventana visible antes/después de Computer Use: no reprodujo la condición original ni aisló su causa. Ese diagnóstico también conserva cierre fallido porque el lanzador terminó la espera antes de acreditar Salir. La repetición final añade hasta ocho segundos para que termine el hijo tras la señal de fin; el proceso ya estaba terminado antes de esa señal. Los fallos anteriores permanecen archivados; no cuentan como cierres correctos.

## Ficha y nota desplegada

[Segundo recorrido acotado](../../.verification/assignment-secondary-contrast/78bdac79-8c2b-4c35-9634-8ef1ae18ca7b/PLAN.json): tres PNG del mismo release y claro real. Ficha con fuente de escritorio ficticia, URL example.invalid, dos pasos y un aspecto de seguimiento; solo lectura/expansión. Editor nuevo con preparación desplegada, cancelado sin guardar. Producto/driver 0, once tablas iguales, ninguna ejecución o trabajo, integridad/limpieza correctas. No se consulta la URL ni se descarga un favicon: el origen es desktop.

[Siete muestras adicionales](../../.verification/assignment-secondary-contrast/78bdac79-8c2b-4c35-9634-8ef1ae18ca7b/MEASUREMENTS.json): URL, resúmenes de preparación/seguimiento, números 1./2., viñeta y nota desplegada; todas 4,96161:1. Los marcadores pequeños aportan nueve píxeles de núcleo cada uno: se registran coordenadas y se exige coincidencia con #525252, la tinta declarada al 80 % sobre #cececd. No se toman bordes suavizados como núcleo por imponer la cuota de dieciséis píxeles de los textos largos. Total de ambos recorridos: **cinco PNG y trece muestras**.

## Límites

Oscuro conserva sus tintas y tiene regresión renderizada, sin nueva captura nativa. La matriz de superficies/condiciones conserva sus límites propios. VoiceOver permanece fuera del alcance por instrucción del usuario. La comprobación de texto no acredita por sí sola los indicadores no textuales de foco del formulario.

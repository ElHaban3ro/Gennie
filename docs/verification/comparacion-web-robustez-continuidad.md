# Comparación web: continuidad y robustez

9 de noviembre de 2026. Una muestra de Gennie y una de Codex directo, secuenciales, sobre el mismo sitio público `quotes.toscrape.com`. No incluye la arquitectura descartada. Modelo `gpt-6.1-terra`, esfuerzo `medium`; Gennie usó el adaptador conversacional y el perfil del bot guardado, con herramientas restringidas para el ensayo. No son la UI completa ni el constructor de plugins. Las configuraciones persistentes no se modificaron.

## Tarea y límites

Los agentes debían generar y ejecutar JavaScript para consultar dos páginas, obtener 20 citas y sus campos, agregar recuentos por autor y etiqueta con autores únicos, generar posiciones filtradas y cantidades por página, entregar el código y explicar su validación. También se pidió que funcionara con tres cambios equivalentes de HTML: orden de atributos, clase adicional y comillas simples. El HTML transformado provino de las páginas públicas reales; un parser independiente comprobó que las tres variantes conservaban los 20 registros. Estas variantes son sintéticas, no cambios realmente observados en el sitio.

Se entregaron las mismas herramientas: lectura de las dos páginas y ejecución de JavaScript Deno aislado, sin acceso a archivos, entorno, procesos o red salvo proxy GET a las dos páginas. Se aplicó un primer error transitorio en `run_javascript`, seguido de respuestas normales; la tarea pedía recuperarse y corregir errores. El máximo fue cuatro invocaciones de código por hilo, incluido el error inyectado. El límite nominal del agente fue 180 s en ambos, con un límite exterior de 210 s y límites idénticos de herramientas. Las fronteras del reloj no son idénticas: Codex empieza a contar desde el inicio de su runner, mientras la sonda Gennie tiene un watchdog iniciado en su prueba y el Scope comienza después de la preparación. Ambos terminaron por debajo del límite. Las muestras son exploratorias, no aleatorizadas.

El verificador construyó el objeto esperado con HTMLParser y comparó su estructura y valores completos. La herramienta devolvió solo un resumen de coincidencia y resultados por variante, sin entregar el oráculo. Los datos tuvieron la misma huella antes y después de cada celda y entre ellas (`dfd2f2333f2929d1b51d64692e06b31d2a5dd831f6c6de17f42ad33bd0062dc6`). Tras terminar, se ejecutó otra vez el código aprobado contra el sitio y, en una ejecución adicional, contra las tres variantes. Esta última reutiliza el mismo generador de variantes y oráculo: es una repetición independiente del agente, no una segunda implementación independiente del verificador.

## Resultados

| Variante | Secuencia de pruebas de código | Validación | Tiempo del runner | Tokens |
|---|---|---|---:|---:|
| [Gennie](../../.verification/real-web-code/556ba034-343b-43a5-a2bc-7a0cade94675/result.json) | Error transitorio → resultado no coincidente → verificado | Sitio y 3/3 variantes; reejecuciones correctas | 102.798 ms | 114.448 |
| [Codex directo](../../.verification/real-web-code/064a7e36-17d7-445c-9b4c-67d4cc7fdc1e/result.json) | Error transitorio → verificado | Sitio y 3/3 variantes; reejecuciones correctas | 64.637 ms | 53.067 |

Ambos terminaron el turno, devolvieron código y declararon su validación. El fallo `mismatch` de Gennie no identifica qué campo discrepó: no se guardó la salida de ese intento. Su tercera llamada aprobó también cada variante, sin agotar el cuarto intento. No se observó un bucle ni abandono en ninguna muestra. La capacidad de recuperación estaba solicitada explícitamente: esto no mide por sí solo proactividad ante instrucciones ambiguas.

El tiempo corre desde el lanzamiento del runner hasta su terminación, incluye arranque, llamadas y pruebas del agente; excluye la preparación previa del oráculo y las reejecuciones independientes posteriores. La reejecución del sitio tardó 685 ms en Gennie y 349 ms en directo; la repetición de variantes tardó 2.279 y 2.273 ms respectivamente. Los tokens incluyen entrada en caché y no representan facturación. Se conservan [repetición Gennie](../../.verification/real-web-code/556ba034-343b-43a5-a2bc-7a0cade94675/independent-robustness.json) y [repetición directa](../../.verification/real-web-code/064a7e36-17d7-445c-9b4c-67d4cc7fdc1e/independent-robustness.json).

## Interpretación

Esta tarea agrega recuperación y robustez a la comparación anterior y duró más de un minuto en cada variante. Es, no obstante, un solo programa sobre un sitio estático de práctica: no demuestra seguimiento de proyectos largos, delegación, compactación ni persistencia entre sesiones. Una muestra no establece tasas ni explica fallos reales del usuario. Ambos cumplieron bajo estas condiciones; la diferencia de tiempo y tokens observada no permite atribuir causalidad al arnés o al modelo. El límite común es nominal y las implementaciones de transporte e instrucciones siguen siendo distintas.

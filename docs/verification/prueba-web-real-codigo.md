# Comparación: código generado y sitio web real

Fecha: 1 de noviembre de 2026. Se hizo una ejecución por variante, sin aleatorización. El objetivo fue escribir JavaScript que consultara las dos primeras páginas de [Quotes to Scrape](https://sites.toscrape.com/), sitio público creado para pruebas de extracción, e imprimiera un array JSON con los 20 registros, en orden, con texto, autor y etiquetas exactos. No se usaron cuentas personales.

## Aislamiento y calidad

El código de los agentes se ejecutó en Deno con lectura/escritura de archivos, variables de entorno, procesos y FFI denegados. Su única conexión autorizada era un proxy temporal de loopback; el proxy aceptaba exclusivamente GET a las dos páginas del sitio real y bloqueaba otros destinos y métodos. Una sonda previa comprobó las restricciones y una solución de control escrita a mano coincidió con el oráculo. El proxy consulta el sitio real en cada ejecución; no es un HTML ficticio, aunque sí es un intermediario que limita el acceso.

Un parser independiente de Python leyó ambas páginas antes de cada celda y construyó el oráculo sin entregárselo a los modelos. Se compararon exactamente los 20 registros completos, sus campos, orden y etiquetas. Después se ejecutó de nuevo el último código aprobado. La huella de los datos esperados fue idéntica en las tres celdas y no cambió entre la lectura anterior y la posterior de cada una (`570637fb4245f74cd18463b56495ae14a4a4573eb1ccb26b2075d788ecb42b83`).

## Resultados

| Variante | Calidad | Tiempo del agente | Validación independiente adicional | Tokens reportados |
|---|---|---:|---:|---:|
| Gennie, sesión conversacional | 20/20 exactos; segunda ejecución correcta | 15.984 ms | 557 ms | 65.079 Codex |
| Codex directo | 20/20 exactos; segunda ejecución correcta | 20.682 ms | 337 ms | 36.792 Codex |
| Arquitectura experimental | 20/20 exactos; segunda ejecución correcta | 73.600 ms | 666 ms | 317.166 Codex + 345 Decisions |

El tiempo del agente comienza al lanzar el runner de esa variante e incluye arranque, generación, consultas de red, pruebas propias y cierre de sus procesos. La columna adicional mide la reejecución independiente; no incluye la preparación inicial del oráculo ni su lectura posterior. La celda Gennie incluye el arranque de Cargo/test y los procesos puntuales de sus herramientas. Son tiempos observados en esta máquina, no tiempos típicos.

Gennie: [resultado](../../.verification/real-web-code/fbf692c7-2db2-4b57-a902-03ab660156fd/result.json) y [código](../../.verification/real-web-code/fbf692c7-2db2-4b57-a902-03ab660156fd/solution.js). Codex directo: [resultado](../../.verification/real-web-code/381961b9-40e9-4041-bd4e-5160cf474226/result.json) y [código](../../.verification/real-web-code/381961b9-40e9-4041-bd4e-5160cf474226/solution.js). Arquitectura: [resultado](../../.verification/real-web-code/b60dfad9-39b8-4459-9b22-d9941721b27f/result.json) y [código](../../.verification/real-web-code/b60dfad9-39b8-4459-9b22-d9941721b27f/solution.js).

Gennie y Codex directo aprobaron en su primera ejecución del código. En la arquitectura experimental, el ejecutor recibió primero un `mismatch`, corrigió el programa y lo verificó; su jefe verificó el mismo código. El jefe registró dos propuestas y ambos Goals terminaron. Los tokens Codex son la suma de los últimos totales de los tres hilos observados; Decisions se suma aparte. Los tokens de caché están incluidos en la entrada y no se suman otra vez.

En su respuesta final Gennie comunicó que la herramienta había verificado el resultado, pero no incluyó el código; Codex directo y la arquitectura sí lo incluyeron. El código de Gennie quedó disponible en la traza de la herramienta y se guardó en el enlace anterior. La herramienta informaba la comparación y el número de filas, pero no devolvía el array completo al agente.

## Comprobación adicional de robustez

Sin volver a llamar a los modelos, se hicieron tres cambios locales al HTML real capturado: reordenar atributos, añadir una clase adicional y sustituir comillas dobles por simples en las clases. El parser independiente confirmó que cada cambio conservaba exactamente los 20 registros. Es una prueba sintética de resistencia, separada de las ejecuciones en vivo.

| Código | Atributos reordenados | Clase adicional | Comillas simples |
|---|---|---|---|
| Gennie | Falla | Falla | Falla |
| Codex directo | Falla | Falla | Falla |
| Arquitectura experimental | Falla | Falla | Pasa |

Los tres programas dependen de expresiones regulares sensibles a la forma del HTML; la coincidencia exacta con la página de ese momento no demuestra resistencia a cambios de estructura. No se corrigió el código generado ni se repitió la inferencia para ocultar esos fallos. [Evidencia de robustez](../../.verification/real-web-code/robustness-863095e8-d26a-4bc7-8961-b5e30708af9c/result.json).

## Alcance de la comparación

La celda Gennie usa el adaptador conversacional Rust real y el texto guardado del perfil del bot del checkout, con modelo y esfuerzo vigentes. Para aislar la prueba se restringieron temporalmente lectura/escritura de archivos, comandos y control de apps y se expusieron solo las dos herramientas del banco; no representa la UI ni el inventario completo de herramientas del producto. Codex directo y la arquitectura recibieron las mismas herramientas y tarea. La arquitectura añadió instrucciones de coordinación, subagentes nativos v1, Goals y OpenAI Decisions. No se cambió el perfil guardado.

El sitio es real, pero su contenido es deliberadamente público, estático y diseñado para pruebas: no hay autenticación, cambios en el sitio ni interfaz web dinámica. Las celdas miden generación y ejecución de JavaScript, **no la creación de un plugin de Gennie**. Una ejecución por variante no basta para afirmar superioridad general, estimar variabilidad, atribuir causalidad al arnés ni calcular coste facturado. No se registraron huellas de todas las fuentes antes de estas tres celdas; la estabilidad del contenido del sitio sí se comprobó como se indica arriba.

Después de las mediciones se cambió el lanzador de Deno para no usar `preexec_fn` en un proceso con hilos y se añadió un límite de 40 solicitudes y cuatro conexiones simultáneas al proxy. Los tiempos de la tabla corresponden a la versión anterior a esos cambios. Tras cambiar el lanzador se volvieron a ejecutar independientemente los tres programas y cada uno coincidió con los 20 registros; sus resultados están en `post-launcher-check.json` junto a cada muestra.

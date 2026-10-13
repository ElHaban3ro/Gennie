# D-020 — Operación musical antes de resolver aplicaciones

2026-10-12. El usuario muestra «Pausa la música.» seguido de «La aplicación solicitada está en primer plano». El diario contiene aperturas verificadas de Apple Music, pero no guarda las frases y no permite vincular una fila con certeza a esa captura. La instancia abierta conservaba un binario anterior; no se reinició durante este trabajo.

## Diagnóstico y cambio

El inventario incluye «Música» entre los nombres localizados de Music. El selector de aplicaciones se ejecutaba antes de interpretar la operación y solo ofrecía abrir esa app u opciones generales. Laya real escogió open_app con 0,9835 para la frase de la captura. En el contexto simulado actual, el filtro posterior de acto de habla la convirtió en ambiguous; no se reprodujo una apertura real. Una regresión de política con ambas respuestas afirmativas sí produjo open_app/com.apple.Music. Otra regresión mostró que incluso una pausa ya interpretada se enlazaba indebidamente con Apple Music por su alias genérico.

Ahora se clasifica primero la operación: apertura, los cuatro controles musicales o una ruta sin efecto. El inventario no interviene en esa pregunta. Las operaciones ejecutables requieren puntuación ≥0,95 y las rutas restantes ≥0,9; una elección incierta no cae al selector de aperturas. Son umbrales experimentales, no probabilidades calibradas ni garantía de acierto. La apertura mantiene además sus filtros de entidad y acto de habla. Un desacuerdo entre apertura y control musical termina en aclaración.

La última frase se clasifica por sí sola. Una apertura anterior no cambia una nueva pausa explícita. El contexto se utiliza para resolver una referencia clasificada como incomplete/ambiguous; la entidad y las aclaraciones conservan sus reglas de contexto. Las adaptaciones existentes mantienen su interpretación supervisada; ninguna se activó ni entrenó aquí.

Para controles musicales, Music y sus traducciones se consideran nombres comunes salvo que indiquen un destino: Apple Music/Música de Apple, un calificador como «en Música», o el nombre aislado como respuesta de reproductor. «Pausa la música» y «Reanuda Music» sin calificador usan el contexto musical comprobado o el reproductor inequívoco del sistema. «Abre Música» sigue abriendo la app. Dos reproductores sin destino inequívoco piden aclaración. No se consulta un reproductor para abrir una aplicación.

## Veto independiente y límites

La evaluación del cambio inicial acertó 8/13 y aún aceptó «Nunca pauses Spotify» y «Ayer pausé la música». No se entregó esa versión. Las comprobaciones semánticas adicionales de Laya tampoco distinguieron de forma fiable todos esos casos.

Se añadió un veto determinista antes de inferir: negaciones explícitas en español/inglés, interrogaciones y comillas no autorizan efecto; marcadores temporales de pasado/futuro y ciertas correcciones con negación requieren aclaración. Este veto solo impide acciones: no reconoce ni autoriza órdenes positivas mediante una lista de frases. Ninguna puntuación del modelo puede anularlo. No es un análisis completo de todos los idiomas o tiempos verbales y puede rechazar peticiones válidas con nombres entre comillas o formulaciones como «no te olvides de…». Estas requieren reformulación; no se promete conservar toda la cobertura anterior.

El ejecutor, comprobación real del efecto, diario y consumo único de autorización conservan sus contratos. No se guardan frases, audio ni nuevas columnas. La política no demuestra fiabilidad general del modelo: sigue vigente el límite de la campaña amplia anterior, que no se repitió en este trabajo.

## Verificación breve

Las dos regresiones se observaron fallar antes del cambio y pasar después. 57 pruebas Python pasan; incluyen protocolo decide→execute, una única orden pause dirigida a Spotify y ninguna apertura, veto previo al modelo, incertidumbre, dos reproductores, destino explícito y contexto anterior.

[`check_music_decisions.py`](../../scripts/check_music_decisions.py) comprueba 13 casos con Laya real y contexto simulado. Resultado final: **13/13 en 5,12 s**, incluida carga, sin efectos, micrófono ni escritura en la DB del usuario. Para los ejemplos negativos mide ausencia de acción, sin afirmar que todas las rutas no ejecutables sean equivalentes. `--exact-only` limita la comprobación a la frase de la captura. [Informe](../verification/music-routing.json).

Cada proceso de comprobación con modelo se limitó a 30 segundos. No se ejecutó una campaña extensa, no se volvió a entrenar y no se probó la pausa sobre Spotify real. `cargo build --locked -p soc` compiló el binario en 5,69 s, dentro del límite de 30 s; conserva el aviso previo de Chat::cancel sin uso. Invariantes documentales, pruebas Python y revisión de espacios pasan. No se reinició soc: reabrir el binario actualizado es necesario para activar el cambio. El aviso de USB Audio CODEC sin audio no se reprodujo ni se modificó en esta intervención.

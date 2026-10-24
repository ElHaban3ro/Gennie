# Auditoría de experiencia de control por voz — 2026-10-12

Se amplió la investigación tras el rechazo de «Abre Discord.». Se corrigieron defectos reproducibles del controlador, inventario, protocolo y selección de reproductor. La batería amplia encontró una limitación semántica pendiente de Laya: los cambios de infraestructura no la resuelven. No se activó ninguna adaptación ni se cambió el modelo, los cortes o las preguntas a partir de los resultados reservados.

## Cambios comprobados

- Cancelar filtra resultados por generación y libera la espera. Una respuesta antigua no puede completar una petición nueva. Se admite una sola orden sin resolver.
- Cancelar trabajo que aún no comenzó conserva la precarga. Interrumpir inferencia descarta el helper y su canal; no declara un fallo de carga. Un fallo real permite recuperarse con la siguiente petición explícita.
- La pérdida del controlador termina la espera una vez y comunica resultado incierto. EOF, JSON inválido, UTF-8 inválido y líneas excesivas no dejan el lector bloqueado.
- El contexto pendiente conserva frases completas recientes hasta 1200 caracteres; su acción caduca junto al texto a los 30 s. No se conserva una intención expirada para una aclaración nueva.
- Laya y Nemotron usan globos independientes. Desactivar órdenes conserva dictado y mensajes terminados; cancelar dictado no elimina una decisión activa.
- Cada mensaje IPC consume la autorización anterior antes de validarse. Un error en una nueva predicción o al comprobar disponibilidad no deja un plan reutilizable.
- El fallback musical respeta el veto de acto de habla. El destino resuelto, un reproductor pendiente y el último destino musical comprobado tienen prioridad sobre el reproductor que casualmente esté sonando; un nombre nuevo explícito prevalece.
- El inventario reconoce `.APP`, recupera nombres válidos frente a metadatos mal tipados/invisibles y rechaza ejecutables que escapan del bundle. Los alias completos prevalecen sobre abreviaturas en el mismo fragmento, conservando ambigüedades reales y menciones separadas. Se encontraron dos nombres invisibles en apps reales de Apple.
- Se rechazan booleanos como probabilidades. La prueba sintética comprueba señal real antes de atribuir un silencio a Nemotron y usa el inventario simulado actual de `open_app`.

Las regresiones Python de inventario, autorización y selección musical se observaron primero fallando, después pasando. Las nuevas regresiones Rust verifican los casos de control/cancelación y pasan sobre la implementación corregida. Resultado técnico: **53 pruebas Rust y 42 Python correctas**, fmt, Clippy con `-D warnings`, build e invariantes. Los adaptadores simulados comprueban política/control, no precisión del modelo ni efecto OS.

## Lenguaje con Laya real

[Corpus](../../scripts/fixtures/voice_experience_cases.json): 220 casos de 44 familias, 166 de desarrollo y 54 reservados; ninguna familia se reparte entre ambos grupos. Incluye puntuación, mayúsculas, espacios, cortesía, negaciones, citas, hipótesis, condiciones futuras, correcciones, destinos múltiples, tareas dentro de apps, contexto pendiente/comprobado y música. Las frases son sintéticas escritas, no transcripciones privadas. El contexto de apps/reproducción es simulado y el contexto conversacional se proporciona explícitamente; no equivale a episodios con historial producido por el sistema.

[Comparación CPU antes/después](voice-experience-comparison.json) y [evaluación final MPS FP32](voice-experience-model.json):

| Medida | Antes | Después |
|---|---:|---:|
| Coincidencia completa de ruta/acción/destino | 62/220 | 62/220 |
| Órdenes ejecutables correctas | 42/83 | 42/83 |
| Selección de una acción cuando no debía ejecutarse | 16/137 | 16/137 |
| Errores técnicos durante evaluación | 0 | 0 |

Desarrollo: 40/166 coincidencias y 14 falsos positivos. Reserva: 22/54 y 2 falsos positivos. GPU reprodujo los mismos recuentos, mediana 81,98 ms por decisión con contexto simulado. No se ejecutó ninguna acción en estas campañas. Una respuesta `ambiguous` frente a `no_order` esperado cuenta como fallo de ruta, aunque no produzca un efecto; los falsos positivos se cuentan aparte.

Ejemplos pendientes: «Mañana abre Discord», «Abre Discord... no, cancela», «Desinstala Discord», operaciones dentro de una app y ciertas descripciones propias pueden seleccionar `open_app`. El ejecutor no adquiere capacidad de desinstalar/borrar, pero sí puede abrir una app indebidamente. Preguntas corteses, aclaraciones cortas y controles musicales también tienen rechazos indebidos. La normalización de puntuación introducida en D-018 puede aumentar la confianza en negaciones complejas; las 31 regresiones iniciales no bastaban para demostrar fiabilidad general.

Se exploraron cuatro preguntas de clasificación y tres comprobaciones semánticas adicionales en desarrollo. Ninguna eliminó los falsos positivos conservando aceptación útil de órdenes simples. No se incorporaron esas variantes ni se eligieron cortes sobre la reserva. El resultado no demuestra control libre fiable con este modelo. Corregir esa parte requiere otra evaluación de interpretación/modelo o adaptación; añadir excepciones a cada frase fallida no constituye una solución general.

## Audio, interfaz y ejecutor

[Voz sintética](voice-experience-synthetic.json): siete audios de `say` con señal comprobada, mono 16 kHz, Nemotron real CPU y Laya en preview. Se transcribieron 6/7 y se obtuvo la salida esperada en 3/7, sin falsos positivos ejecutables. «No abras Chrome» se transcribió «No habrá Scrum»; «abre el» produjo «Abreel»; «hazlo» no produjo texto. No se corrigieron manualmente las transcripciones. Una primera ejecución restringida produjo resultados vacíos y avisos del sandbox; se invalidó y se repitió con acceso local.

[Ejecutor nativo](voice-experience-native.json): «Abre Finder.» desde el binario actualizado, SQLite temporal, aplicación realmente en primer plano, resultado `verified` y foco anterior restaurado. No se reinició la instancia habitual de gennie.

`chat_probe --decisions` ejecutó una secuencia visual aislada de 6,6 s con diez snapshots: carga/listo/análisis y resultados de Laya mientras Nemotron cambia de estado. [Coexistencia a 1,2 s](voice-experience-chat-1200.png) y [reposo sin globos a 6,3 s](voice-experience-chat-6300.png). La sonda restringida quedó bloqueada por el entorno; se verificó su PID y se cerró exclusivamente ese proceso propio antes de finalizar. La ejecución nativa terminó por sí misma. Los mensajes son simulados: esto no prueba micrófono ni gesto físico del atajo.

Precarga desde Controller con contexto real bajo sandbox: 4595 ms, primera decisión 1294 ms, sin efecto. El inventario legible bajo sandbox contenía 237 apps; los 237 nombres conservaban el destino entre sus candidatos y nueve tenían ambigüedad real. No comparar ese recuento limitado con una enumeración irrestricta como si se hubieran eliminado apps.

## Reproducción

- `python3 -m unittest discover -s apps/decisions/python`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Con el Python configurado para Laya: `scripts/evaluate_voice_experience.py --model RUTA --output INFORME --split all`. `--trace` incluye preguntas/puntuaciones de los casos sintéticos. Sale con código 1 mientras haya discrepancias: no es una suite semántica verde.
- `scripts/review_voice_pipeline.py --nemotron RUTA --output TRANSCRIPCIONES`, seguido de `scripts/evaluate_voice_decisions.py --model RUTA --fixture TRANSCRIPCIONES --output INFORME` con el Python de Laya.
- `cargo run --locked -p gennie-presence --example chat_probe -- CARPETA --decisions` para la secuencia visual propia.
- `scripts/probe_decisions_native.py` abre Finder, usa SQLite temporal y restaura foco; es una prueba con efecto explícito, distinta de los evaluadores anteriores.

La voz real del usuario, el atajo físico, permisos de micrófono y todas las apps posibles siguen sin quedar demostrados por estas pruebas. Reabrir el ejecutable compilado es necesario para aplicar los cambios a una instancia existente.

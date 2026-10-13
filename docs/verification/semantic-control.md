# Verificación de D-017 — 2026-10-12

La revisión posterior del fallo concreto «Abre Safari.» está en [regresión del router](safari-router.md); las mediciones del modelo base que siguen corresponden a antes de esa corrección.

Estado: implementación de intención/disponibilidad/contexto y herramientas de adaptación/revisión. Los pesos candidatos no se equiparan a una mejora hasta medirlos. El usuario pidió dejar preparada la prueba con su voz; no se capturó micrófono en esta sesión.

El CSV/JSONL proporcionado contiene 6.278 filas coincidentes, con IDs únicos. La partición nueva agrupa variantes de una misma frase antes de asignar entrenamiento/calibración/prueba; el comprobador verifica grupos disjuntos. Entrenamiento usa controlled y anotaciones explícitas adicionales. Las 210 frases independent no se entrenan, pero fueron inspeccionadas anteriormente y no son una prueba ciega.

Primer candidato, encoder congelado: 455/567 clases correctas en prueba, pero cero acciones aceptadas al umbral escogido en calibración. No activado.

Segundo candidato, última capa y cabeza de representación: 496/689 clases correctas en prueba; 166 acciones aceptadas, tres acciones equivocadas y cero falsas selecciones para ejemplos no ejecutables. Calibración: 965/1324, 418 aceptadas, cuatro equivocadas. No activado como sustituto automático. No se confunde el cero de derivaciones erróneas con cero errores entre acciones locales.

Tercer candidato: entrenamiento de la pregunta tipada y scorer originales de Laya, con encoder base intacto en disco. Clasificación sin corte: 621/689 (90,1%). Corte global elegido en calibración: 54 acciones aceptadas, cero equivocadas en prueba. Cortes por acción elegidos en calibración: 187 aceptadas, dos equivocadas, ambas para ejemplos no ejecutables. La comprobación adicional de orden/operación con los pesos originales de Laya deja 17 aceptadas de 689, cero equivocadas; en calibración deja 14 de 1324. Este filtro usa el mismo modelo con distintas preguntas/pesos, no un verificador independiente.

En las 210 frases independent, el perfil por acción más comprobación semántica obtuvo 76 coincidencias según la comparación histórica (todas las rutas sin ejecución se agrupan como god_call). Emitió cuatro acciones, una incorrecta: «solo dime si entendiste: abre Discord» fue seleccionada como abrir Discord. Son selecciones simuladas: no se ejecutó ninguna. El cero observado en los 689 casos no garantiza ausencia de errores fuera de ese conjunto. Esta campaña fue exploratoria; ajustes sucesivos observando resultados impiden presentarla como validación ciega final.

Laya base con las preguntas semánticas nuevas: 74/210 coincidencias históricas, cero acciones emitidas (58 ambiguous, 63 god_call, 36 incomplete, 53 no_order). Esta abstención general no constituye cobertura útil. [Resultados](semantic-base-challenge.json).

No se instaló ni activó ninguna adaptación, ni se escribió runtime-v2.json. Los artefactos quedan en `.verification/decision-domain{,-tuned,-typed}`. El perfil tipado conserva manifest, split, typed.safetensors, calibration-predictions.json y gate-evaluation.json. Base SHA-256: `9d628fd971b700382ac6f65920a86f149777b2e748e0c955fb3b19695aa8f204`; adaptación tipada: `ebd9ae61e19e78f87ea547008d58ac1393d87d1033285f387e111a628032a4da`. La cobertura fiable sigue pendiente; estos pesos son material de evaluación, no una actualización recomendada del controlador.

Pruebas de código: disponibilidad ausente no altera la intención ni llega al ejecutor/diario; acción pendiente pasa a la aclaración; contexto comprobado caduca; adaptación permite frases sin whitelist; reproductor explícito se conserva; respuestas fuera del catálogo, cancelación, replay y evidencia siguen comprobados.

Audio sintético: siete frases generadas con Monica, convertidas a mono 16 kHz y procesadas por Nemotron CPU. Sin silencio añadido hubo tres transcripciones vacías y algunas palabras perdidas. Con 300 ms de silencio inicial y 1,5 s final se reconocieron seis de siete; «hazlo» quedó vacío. «no abras Chrome» se reconoció como «No habrá Scrum», y «abre el» como «Abreel». Se conservan estas salidas, no se sustituyen por el texto esperado. Es un hallazgo del camino de streaming/voz sintética, no una causa demostrada de fallos del micrófono ni una modificación de audio de producción.

Las seis transcripciones no vacías se entregaron al perfil tipado en preview: una de las siete salidas completas coincidió con la ruta/acción esperada y cero acciones para ejemplos no ejecutables. Las tres órdenes positivas quedaron en aclaración. Evidencia: [audio → decisión](semantic-synthetic-voice.json), [frases difíciles](semantic-challenge.json). La transcripción vacía se conserva como no_speech y no se envía a Laya.

La revisión física se ejecuta después con `cargo run --locked -p soc -- --voice-review`: SQLite en memoria, Ctrl+Option+Shift+R, solo preview, tres minutos y cierre propio. La entrada es la predeterminada y los permisos pertenecen al ejecutable realmente iniciado. No garantiza un atajo/dispositivo/permisos que aún no se han observado.

Reproducir:

```sh
python3 -m unittest discover -s apps/decisions/python -p 'test_*.py'
python3 scripts/check_decision_library.py --jsonl /ruta/library_all.jsonl --csv /ruta/library_all.csv --split .verification/decision-domain-typed/split.json
/ruta/.venv/bin/python scripts/train_decisions.py --model /ruta/multilingual --library /ruta/library_all.jsonl --output .verification/decision-domain-typed --finetune --typed --epochs 3
/ruta/.venv/bin/python scripts/evaluate_decisions.py --model /ruta/multilingual --adapter /ruta/adaptacion --library /ruta/library_all.jsonl --dataset challenge --limit 210 --output /tmp/soc-review.json
/ruta/.venv/bin/python scripts/evaluate_decision_gate.py --model /ruta/multilingual --library /ruta/library_all.jsonl --output .verification/decision-domain-typed
python3 scripts/review_voice_pipeline.py --nemotron /ruta/nemotron.gguf --output /tmp/soc-voice-review.json
/ruta/.venv/bin/python scripts/evaluate_voice_decisions.py --model /ruta/multilingual --adapter /ruta/adaptacion --fixture /tmp/soc-voice-review.json --output /tmp/soc-voice-decisions.json
```

No hubo reinicio de la instancia ajena ni cambios en el checkpoint original. Compilación, inferencia de texto, inferencia de audio sintético y prueba física de voz son evidencias distintas.

Verificación final del código: 41 pruebas Rust y 15 Python; cargo fmt, test, clippy con warnings como errores y build --locked completados. Comprobador documental y correspondencia CSV/JSONL con grupos disjuntos correctos. La evidencia de ejecución nativa de D-016 pertenece al controlador anterior; esta campaña no prueba efectos del nuevo intérprete semántico en apps reales.

Sonda final Rust → helper real → Laya base: `target/debug/soc --decide "abre Finder"` devolvió ambiguous (0,3221), verified=false. SQLite en memoria, sin ejecución ni micrófono; el helper terminó. Confirma el recorrido IPC y también la falta de cobertura en una orden sencilla. La instancia existente PID 63965 continuó activa.

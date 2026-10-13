# Verificación del control local — 2026-10-12

Evidencia histórica de D-016; la campaña vigente se documenta en [D-017](semantic-control.md).

Fuente: library_all.jsonl / library_all.csv del directorio `/Users/fer/Documents/Codex/2026-10-12/s/outputs/desktop-model-comparison`. Son 6.278 frases escritas, no grabaciones del micrófono ni una muestra de habla futura. Se leyó el README como referencia y se siguió el diagrama/petición del usuario.

`scripts/check_decision_library.py` comprueba correspondencia CSV/JSONL y la autorización independiente sobre todo el corpus: 4.499 elegibles y cero autorizaciones de una acción distinta del gold. Sin inferencia ni efectos: no equivale a 100 % de precisión del sistema. Las frases restantes no reciben autorización automática con el filtro actual.

Diez pruebas unitarias Python cubren las cinco salidas, correcciones/continuaciones, citas/negaciones/órdenes múltiples, acción fuera de opciones, reproductores ambiguos, contexto cambiado, evidencia y envío único tras fallo. Rust prueba predicciones mal formadas, diario, persistencia/reapertura, arranque/cierre idempotente, recuperación de intentos pendientes, no reutilizar fallos como contexto y cancelación/cierre de un hijo simulado antes del efecto.

Se midieron iteraciones con Laya real, MPS y contexto de escritorio simulado. La primera versión libre acertó 26/120 y seleccionó 5 acciones indebidas; otra versión 50/120 y 6; filtro con catálogo todavía amplio, 68/120 y cero selecciones indebidas. Son muestras de desarrollo; influyeron en el diseño y no se presentan como validación independiente.

La última evaluación del desafío escrito obtuvo 193/210 acciones correctas y cero selecciones ejecutables indebidas, con contexto simulado. El filtro se amplió después de inspeccionar errores; no es una validación independiente sin ajustes. La evaluación del desafío escrito se guarda en [decisions-challenge.json](decisions-challenge.json). Contexto simulado con todas las apps instaladas y un reproductor, last_action/pending_app proporcionados por el corpus. No demuestra el contexto predicho de una conversación real. Contiene cada frase, respuesta, tiempo y expectativa; ninguna acción se ejecuta. El corpus original agrupa negaciones/incompletas/ambigüedades como god_call: para comparar acciones, las cuatro rutas sin ejecución equivalen a god_call; eso no evalúa por sí solo la exactitud de las cinco rutas.

Lectura nativa real comprobada: NSWorkspace detectó Safari como navegador predeterminado, Chrome y Launchpad ausentes; no se abrió ninguna aplicación para esa lectura. No se solicitó micrófono, Accesibilidad ni Apple Events en esas pruebas. Los efectos musicales se prueban con adaptador simulado; no se declara control real de Spotify/Música hasta probarlo con permisos y reproducción reales. Tampoco se ha observado todavía el ciclo voz→acción desde el atajo físico ni el nuevo control de ajustes en UI.

Reproducción desde raíz, usando el Python que contiene Laya:

```sh
python3 -m unittest discover -s apps/decisions/python -p 'test_*.py'
python3 scripts/check_decision_library.py --jsonl /ruta/library_all.jsonl --csv /ruta/library_all.csv
/ruta/.venv/bin/python scripts/evaluate_decisions.py --model /ruta/multilingual --library /ruta/library_all.jsonl --dataset challenge --limit 210 --output /tmp/soc-decisions-review.json
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --locked -p soc
python3 scripts/check_invariants.py
```

No sustituir el control por voz por una promesa de lenguaje libre: el filtro está acotado al español y su cobertura sigue siendo una limitación explícita. god_call termina con integración pendiente, como pide el diagrama.

La primera prueba del binario falló antes de actuar con ModuleNotFoundError: laya. Causa demostrada: configure_laya resolvía el enlace del intérprete hasta el Python base, perdiendo el venv. Una prueba que ejecuta el configurador con un enlace simbólico falló antes del arreglo y pasó después. El configurador conserva ahora la ruta absoluta del enlace; el runtime local se volvió a configurar.

Prueba nativa del binario: `python3 scripts/probe_decisions_native.py` ejecutó «abre Finder» con SQLite temporal. Laya devolvió complete/open_files (0.833), NSWorkspace observó Finder en primer plano y el diario persistió verified. Se restauró el foco anterior y se eliminó la DB temporal. Informe: [decisions-native.json](decisions-native.json). Esta prueba ejercita Rust→Python→Laya→OS→SQLite; no incluye micrófono ni demuestra los otros ejecutores.

Checks finales: 39 pruebas Rust y 10 Python pasan; fmt, clippy con warnings como errores, build locked, invariantes documentales y diff check pasan. Se preservan los cambios previos del checkout; no se cambió rama ni se reinició/empaquetó la instancia del usuario.

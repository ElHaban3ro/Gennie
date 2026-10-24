# Regresión del router — «Abre Safari.»

2026-10-12. Captura del usuario: transcripción completa «Abre Safari.» y respuesta de gennie solicitando continuar la frase.

Reproducido dos veces con `target/debug/gennie --decide 'Abre Safari.'`: incomplete, puntuación 0,4485. La assertion complete/open_safari falló. SQLite en memoria, sin micrófono ni efectos. La instancia visible ejecutaba el binario anterior a esta corrección, sin adaptación configurada.

Sondas sobre el mismo checkpoint/contexto mostraron dependencia de las claves de opciones: con nombres semánticos la ruta fue complete (0,7558) y la operación open_safari (1,0). El router además aceptaba cualquier argmax no ejecutable como una clasificación firme, aunque su puntuación fuese baja. El texto no se perdía en IPC ni ASR.

Corrección: claves semánticas, orden preservado, ruta incierta sin afirmación de frase incompleta; aceptación completa exige apoyo ≥0,7 de ruta y ≥0,9 de operación. Sin gramática especial ni cambios de modelo/ejecutor. La operación identificada se conserva para aclaraciones cuando no alcanza el corte.

Después de compilar, la misma assertion en Rust → helper Python → Laya real pasa: complete/open_safari/com.apple.Safari, disponible, puntuación 0,7558. verified=false y «Predicción disponible; sin ejecución». [Salida del runtime](safari-runtime.json). No se abrió Safari ni se comprobó un efecto OS.

Campaña de 210 frases independent, contexto simulado, sin efectos: 81 coincidencias históricas, siete acciones emitidas correctas, cero selecciones falsas para ejemplos god_call. Las demás: 155 ambiguous, 46 god_call y dos no_order. [Resumen](safari-challenge.json). No es validación ciega ni cobertura general suficiente; el cambio resuelve la frase exacta observada.

Regresión real complementaria: `scripts/check_decision_regressions.py --model /ruta/multilingual --output /tmp/regressions.json`, órdenes de Safari/Finder, negación, cita, pasado, frase incompleta y búsqueda. [Resultados](safari-regressions.json): 7/7 correctos para aceptación/rechazo. Las rutas exactas de algunas frases sin efecto siguen siendo incorrectas (por ejemplo, pasado → god_call); esta regresión no las presenta como resueltas. Pruebas simuladas también cubren el argmax incomplete de 0,4485 sin esperar más voz indebidamente.

Compilación y pruebas se distinguen de activación: la instancia ajena no se detuvo ni reinició. El helper de preview terminó. Para recibir el nuevo router hay que cerrar y volver a abrir gennie con el binario actualizado; no hay hot reload.

Checks finales: 41 pruebas Rust, 16 Python, fmt, clippy sin warnings y build --locked correctos. El comprobador documental pasó después de guardar los resultados reales enlazados.

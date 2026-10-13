# D-017 — Intención semántica, contexto y adaptación de Laya

2026-10-12. El usuario autorizó implementar los cinco puntos de mejora de D-016. Esta decisión reemplaza su autorización por gramática de frases: el router ya no contiene una lista de formas admitidas ni el archivo policy.py. Conserva catálogo cerrado, ejecución única, comprobación del OS y diario.

La clasificación previa de operación, la referencia genérica a música y el veto independiente de efectos se actualizan en [D-020](music-routing.md). No hay una lista de órdenes positivas admitidas; sí un veto conservador de señales negativas/temporales que el modelo no puede anular. Las métricas de esta página son históricas.

## Intención y disponibilidad

Primero se interpreta el texto completo con contexto, sin limitar las opciones a apps instaladas. El resultado semántico es una de las doce acciones o incomplete/ambiguous/no_order/god_call. Una acción implica intención complete. Después se resuelve el destino y su disponibilidad. «Abre Chrome» con Chrome ausente mantiene complete/open_chrome y devuelve availability=unavailable, no una ambigüedad ni una sustitución por Safari. El controlador comunica la aplicación ausente y no crea intento de ejecución.

Las entidades de reproductor se enlazan después de interpretar la operación. Spotify/Música expresamente nombrados se conservan; una corrección que menciona ambos necesita selección semántica. Sin selección expresa se usa el reproductor inequívoco del sistema. Dos reproductores ambiguos mantienen requested_action y piden cuál controlar. El destino/contexto se vuelven a comprobar antes del efecto. El helper solo acepta execute para la intención que acaba de emitir, y la consume una vez.

El modo sin adaptación reformula las preguntas de Laya: intención final y operación del catálogo, con contexto y correcciones, independientes de instalación. Las decisiones inciertas piden aclaración. La adaptación usa una pregunta tipada de dieciséis respuestas: doce operaciones y cuatro salidas sin efecto. Los nombres de clases/catálogo son límites de capacidad, no frases que el usuario deba memorizar.

Revisión del fallo «Abre Safari.»: el modo base usa claves semánticas de ruta/acción, conservando el orden del catálogo. Las claves opacas A–E desviaban esa petición hacia incomplete (0,4485). Una ruta no ejecutable con puntuación inferior a 0,9 se convierte en ambiguous; no afirma que faltan palabras. Para complete se exige ruta ≥0,7 y operación ≥0,9, dos distribuciones diferentes. Estos cortes son una política experimental comprobada en la regresión, no probabilidades calibradas de éxito ni una garantía general. No se añaden excepciones por frases/aplicaciones. Ver [regresión](../verification/safari-router.md).

## Contexto

Última acción y destino comprobados solo se actualizan tras OS verificado y commit; caducan a los cinco minutos. Los fallos no actualizan este antecedente. La petición pendiente conserva texto y requested_action hasta 30 s y permite responder al pedido de aplicación/reproductor. Una aclaración sin nueva acción conserva esa acción pendiente. Cancelar limpia pendientes por generación; no reproduce trabajos antiguos. No se guarda conversación/audio en DB.

La evaluación de episodios debe usar resultados predichos/comprobados del episodio, no last_action gold, salvo una prueba explícitamente etiquetada como contexto proporcionado. Un preview no inventa una acción comprobada para el siguiente turno.

## Entrenamiento y separación

`scripts/train_decisions.py` entrena sobre el corpus controlled proporcionado y anotaciones explícitas adicionales para las cuatro salidas nuevas, referencias y aclaraciones. El corpus antiguo agrupa varias salidas como god_call: su familia de negaciones se anota no_order, y missing_context se anota ambiguous. Otras derivaciones conservan god_call; no se inventa que cada antigua derivación sea una ambigüedad.

Variantes de una misma frase central comparten grupo, incluso al cambiar muletillas/contexto. La partición determinista asigna grupos completos a entrenamiento, calibración o prueba. `check_decision_library.py --split` comprueba que no haya un grupo en varias particiones y que CSV/JSONL correspondan. Las 210 frases independent se evalúan aparte; ya habían sido inspeccionadas en D-016 y no se presentan como un examen ciego nuevo.

Se implementaron y ejecutaron tres experimentos: cabeza sobre representaciones congeladas; ajuste de la última capa con cabeza de clasificación; ajuste de la última capa y de head/scorer originales sobre la pregunta tipada. El checkpoint original nunca se sobrescribe. Pesos de adaptación se guardan como safetensors, con hashes del modelo base y de los archivos; no pickle ni ejecución de contenido del artefacto. Manifest declara clases, umbral, particiones, tipo de adaptación y métricas. Los umbrales se seleccionan en calibración; la prueba no se usa para elegirlos. Probabilidad/umbral no garantizan precisión futura.

Los primeros dos experimentos no demostraron una mejora suficiente: la cabeza congelada aceptó cero acciones en la prueba; el ajuste de representación aceptó 166/689, con tres acciones equivocadas. El tercer experimento obtuvo 621/689 clases correctas, pero sus políticas de aceptación no demostraron cobertura fiable: la comprobación adicional solo aceptó 17 y aún produjo una selección falsa en las 210 frases difíciles. Ninguna adaptación se activó. Los resultados definitivos se registran en [verificación](../verification/semantic-control.md). Entrenar un modelo no significa que deba sustituir automáticamente al anterior.

## Configuración y procesos

La configuración nueva puede incluir adapter, ruta absoluta del directorio con manifest/pesos. `configure_laya.py --adapter` escribe runtime-v2.json por defecto. El binario nuevo prioriza ese archivo; el antiguo sigue usando runtime.json. Esto evita romper una instancia antigua en ejecución por su deny_unknown_fields. El intérprete conserva el enlace del venv. No se cambia la DB ni el entorno global ni se reinician instancias ajenas.

Laya/PyTorch y safetensors son los ya instalados; no se agregan paquetes ni descargas. Modelo base, GPU y helpers viven fuera de kernel. Worker e IPC mantienen sus límites/cancelación. No se modifica una migración aplicada.

## Voz

`--voice-review` usa SQLite en memoria y Ctrl+Option+Shift+R, distinto del atajo normal. Preview obligatorio: no ejecuta, no guarda audio/transcripción y finaliza a los tres minutos, cerrando recursos propios. Usa la entrada predeterminada y requiere los permisos reales del proceso; no prueba el dispositivo elegido en otra instancia. El usuario pidió dejar esta prueba preparada para después; no se abre micrófono en esta sesión.

`review_voice_pipeline.py` usa say a archivo, conversión mono 16 kHz y streaming_probe CPU para evaluar Nemotron sin micrófono ni efectos. Se conserva su transcripción real, sin corregirla manualmente, y luego puede entregarse a Laya en preview. El silencio sintético añadido para una segunda prueba no se incorpora a la captura de producción. Audio vacío no se interpreta como orden. Estas pruebas no equivalen a la voz/atajo físicos.

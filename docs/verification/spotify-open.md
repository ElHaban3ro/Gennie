# Spotify abierto con error de comprobación

2026-10-12, hora de Colombia. La captura del usuario mostró un error genérico tras «Por favor, abre Spotify.», aunque la aplicación se abrió. El diario personal, consultado sin modificarlo, contenía un intento uncertain sin evidencia y aperturas posteriores verified. No se conservó la excepción del incidente, por lo que su causa exacta no puede atribuirse retrospectivamente.

## Hallazgos y corrección

Una prueba controlada reprodujo el síntoma: un error en la primera lectura posterior a open abortaba la respuesta aunque la apertura ya se hubiera enviado. Un timeout de open también evitaba comprobar su efecto. Las regresiones fallaron antes del arreglo.

La primera sonda nativa de esta revisión encontró además `KeyError: 'frontmost'` en su lectura posterior: JXA había omitido ese campo. La sonda antigua no guardó el resultado completo al fallar. La lectura ahora representa identidad ausente como null; nunca la usa para confirmar éxito. La sonda conserva salida y diario aunque falle una observación posterior.

El ejecutor envía open una sola vez y comprueba running/frontmost con hasta 12 lecturas y un plazo de 4 s, máximo 1 s por lectura. Recupera errores transitorios sin repetir la apertura. Sin confirmación, registra uncertain con diagnóstico por etapa, conserva Laya y comunica que no pudo comprobar el resultado. Un fallo al crear el proceso se registra failed. Ninguno se convierte en contexto exitoso. Las excepciones restantes diferencian carga, inferencia y ejecución mediante categorías fijas, sin exponer stderr ni argumentos.

## Evidencia y límites

- 50 pruebas Python pasan: lectura ausente, recuperación, timeout, plazo acotado, fallo persistente, fallo de creación y aplicación en ejecución sin primer plano.
- 62 pruebas Rust pasan, incluidas persistencia uncertain, ausencia de repetición/contexto exitoso y modelo Ready. Fmt y clippy pasaron; el binario se volvió a compilar después del último cambio Python.
- La [prueba completa con Laya y SQLite temporal](spotify-open-native.json) clasificó complete/open_app/com.spotify.client (0,9997). Registró uncertain ante un timeout de observación y ausencia de Spotify en primer plano; la lectura posterior mostró Safari. **Esta prueba no pasó su assertion de apertura verificada** y no se presenta como éxito integral.
- La [prueba directa del ejecutor](spotify-open-executor.json) abrió Spotify una sola vez, inyectó un fallo en la primera comprobación y después observó Spotify en primer plano: verified. Restauró el foco. La apertura y las lecturas posteriores fueron reales; el fallo inicial fue simulado. No incluyó modelo ni DB.

Se detuvieron las pruebas adicionales por petición del usuario. Las pruebas largas futuras requieren aviso de propósito, alcance y duración estimada, y visto bueno previo. No se reinició gennie, no se cambió la DB personal y no se probó el micrófono. La instancia abierta requiere reabrir el binario actualizado para incorporar los cambios.

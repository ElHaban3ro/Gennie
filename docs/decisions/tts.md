# D-022 · Voz de salida de SOC

SOC reproduce las respuestas finales de la conversación y del control local mediante Kokoro-82M con MLX. La síntesis y reproducción son locales, sin puerto ni API de voz remota. El texto permanece visible en el globo. Un fallo TTS se muestra en los ajustes y en un aviso temporal sin perder la respuesta escrita.

`apps/tts` posee `tts_preferences` (activado por defecto, `ef_dora`, velocidad 100 %) y un worker serial con cola acotada. Las voces iniciales son `ef_dora`, `em_alex` y `em_santa`; velocidad 70–130 %. «Probar voz» usa la selección aún no guardada. Guardar confirma después del commit; apagar lectura no desactiva conversación. El proceso Python permanece cargado entre respuestas y no persiste texto o audio. `stop` mata y espera ese proceso.

El entorno Python 3.12 y el caché del modelo viven en `~/Library/Application Support/soc/tts`. La primera inferencia descarga pesos; las siguientes usan el caché. La app no instala paquetes en segundo plano. El worker divide cada envío en frases de hasta 160 caracteres, sintetiza el primer fragmento antes de abrir la salida y reproduce los siguientes mientras se generan, con un único stream de `sounddevice`. La conversación también entrega frases terminadas conforme llegan fragmentos del proveedor; conserva la última frase incompleta hasta que se cierre o finalice el mensaje. Las frases esperan en orden si la cola de Kokoro está llena, sin bloquear el hilo visual. [Fuente del motor](https://github.com/gabrimatic/kokoro-mlx).

Al pulsar el atajo para iniciar dictado, presence interrumpe la reproducción actual, descarta las frases aún en cola y no vuelve a narrar fragmentos posteriores de ese turno interrumpido. La cancelación mata únicamente el proceso hijo TTS; una respuesta futura lo inicia de nuevo y recarga Kokoro. La prueba del worker cubre audio en este Mac; la interacción de la ventana completa requiere comprobar una instancia nueva del binario.

Cada reproducción lleva un identificador y emite inicio, fin o fallo. Presence conserva la respuesta visible y la cara despierta desde que la encola, aunque Kokoro tarde en cargar; al acabar, inicia el plazo configurado de visibilidad. Varias reproducciones pendientes impiden el sueño hasta completarse todas. La prueba de voz aplica la misma regla de vigilia, sin crear un globo.

El texto enviado a Kokoro se prepara desde la misma respuesta que alimenta el globo: se retiran marcas Markdown y enlaces de fuentes. El contenido original puede conservarse en el historial de texto; el audio nunca lee las URLs citadas.

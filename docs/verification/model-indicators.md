# Indicadores de modelos — 2026-10-12

Dos iconos bajo la cara distinguen Nemotron (micrófono, reconocimiento de voz) y Laya (destello, análisis de órdenes). El relleno depende de ModelStatus::Ready de cada controlador. El tooltip identifica función y estado; una descarga de archivo, un modelo cargado y una sesión escuchando son estados distintos.

La sonda `model_indicators_probe` terminó correctamente en macOS con Winit/FemtoVG, pantalla 1920×1080 a escala 1, sin audio, DB, modelos ni acciones. Guardó nueve capturas en `.verification/model-indicators-isolated`. La revisión visual confirmó contorno/relleno en cara dormida/activa, ambos tooltips legibles y desaparición al salir. Comparaciones RGBA: 128 píxeles cambiaron al cargar voz; los tooltips cambiaron 7232/5684 píxeles y salir devolvió exactamente la imagen previa (0 diferencias).

Los estados y el puntero de esa secuencia son sintéticos. Se aislaron de los eventos físicos porque la primera pasada había mezclado ambas fuentes y no mostraba tooltips de forma reproducible. La sonda cerró su ventana y proceso. Esto no demuestra hover físico, una nueva carga real de modelos, otros monitores/Spaces ni activación del cambio en una instancia de soc ya abierta.

Las pruebas de controlador cubren archivo verificado frente a carga, conservación después de NoSpeech/cancelar sesión y liberación tras cancelar el controlador o fallar. Las pruebas de presentación comprueban que solo Ready rellena; las geométricas cubren círculo, ambos iconos, separación y zonas transparentes. macOS además envía PointerExited al salir de las áreas interactivas, fuera del acceso Winit, para no retener un tooltip si AppKit omite la salida al activar click-through.

Resultado final: 59 pruebas del workspace correctas; fmt, clippy para todos los targets con warnings como errores, build de soc y comprobación de invariantes correctos. Binario compilado, sin reiniciar la instancia del usuario.

Procedimiento reproducible: [reglas de pruebas](../rules/testing.md). Comportamiento: [presencia nativa](../rules/frontend.md).

Variante posterior solicitada: iconos sin contenedor visible, conservando área de hover, paths y relleno. Build de soc y sonda, fmt e invariantes correctos. Nueva secuencia nativa aislada en `.verification/model-indicators-no-container`: nueve capturas, relleno y tooltips presentes, desaparición con 0 diferencias respecto a la imagen anterior. Inspección visual de cara activa confirma ausencia de fondo/borde; se mantienen los límites de entrada sintética indicados arriba.

2026-10-14 — Kokoro añade un tercer icono de altavoz. El worker Python emite confirmación al cargar el modelo y `soc-tts` expone su estado a presence; solo Ready rellena el icono. La sonda de renderer de software confirmó tres indicadores alineados, huecos entre ellos y el botón de chat, y 106 píxeles cambiados en el altavoz al marcar Kokoro cargado. Se inspeccionó la captura. La sonda Winit nativa no llegó a abrir su ventana en este entorno por un fallo de conexión con el servicio gráfico de macOS; se cerró el proceso temporal. El render aislado no prueba hover físico, audio real ni la instancia activa de SOC.

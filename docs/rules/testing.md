# Verificación proporcional

Anunciar propósito, alcance y duración antes de pruebas largas y obtener el visto bueno. No iniciar modelos, consumir suscripción, abrir ventanas ni alterar procesos ajenos para una comprobación estática. Un único proceso Cargo por target; no ejecutar scripts dev/build simultáneos.

## Comandos

Desde la raíz: `python3 scripts/check_invariants.py`, `cargo fmt --all -- --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings`. Elegir primero los tests afectados. Desde frontend: `flutter analyze` y `flutter test`. `./scripts/dev.sh --build-only` compila Rust y resuelve paquetes, sin compilar runner ni abrir DB/UI. Regenerar el puente con `./scripts/dev.sh --generate --build-only` si cambia la API.

## Escritorio Flutter

Tooltips: `scripts/check_invariants.py` rechaza `Tooltip` y propiedades `tooltip` ajenas a `AppTooltip` (salvo el valor vacío que desactiva el tooltip interno de menús). Los widgets verifican hover y cierre tanto en navegación como en acciones, y el mismo canal nativo macOS con colocación según el control. Esta cobertura no sustituye la comprobación visual del material nativo.

Comprobar con widgets navegación, selección de lectura, estados pendientes, errores con borrador conservado, nombre local, eliminación y datos recibidos por API. Los fixtures no prueban backend real. No se ejecutan las sondas visuales de la interfaz retirada ni sus ejemplos eliminados.

Para evidencia nativa se requiere Xcode completo y una instancia propia con DB temporal: abrir, redimensionar al mínimo, navegar por teclado, revisar claro/oscuro, scroll, composición Markdown, enlaces, errores y ajustes. Cerrar ventana debe ocultarla y mantener servicios; Salir debe cerrar coordinador, hijos y SQLite mediante el canal `gennie/lifecycle` (`requestQuit`). No reiniciar una instancia del usuario. El paquete release debe contener dylib, helper y licencia Inter. Build correcto no prueba Dock, Stage Manager, foco, vidrio ni permisos físicos.

Xcode 27.1 está disponible y el runner release compila. Mantener separadas las pruebas Dart, Rust, estáticas y runtime. Hot reload Dart tampoco prueba cambios Rust; recompilar la biblioteca. Las pruebas integrales de micrófono, atajo, Codex y audio requieren observación de la nueva aplicación; evidencia histórica no basta.

## Persistencia y cierre

DB temporal para verificar migraciones intactas, reapertura, aislamiento de chats, activación atómica local/proveedor, nombres Unicode, borrado con rollback, respuesta tardía sin resucitar filas y confirmación después del commit. No guardar audio, parciales ni estados temporales. Un fallo de persistencia debe verse y conservar el borrador; detener workers antes de cerrar DB. Probar errores/cancelación sin ejecutar efectos externos.

## Modelos y servicios

Preferir reloj simulado y adaptadores aislados para timeout, precarga y cierre. Sin descargas ni modelos reales en tests automáticos. Un mock no demuestra micrófono, TTS audible, cuenta ChatGPT ni ejecutores OS. Las pruebas reales deben tener duración y efectos acotados, DB propia y cierre de todos los procesos creados. Recordatorios requiere permiso EventKit y no está conectado a la conversación/UI.


## Conversación LLM
Verificar con DB temporal que el `thread_id` persiste al reabrir y no cambia entre frases. Nuevo chat debe cambiar el ID solo por petición explícita, persistirlo antes de informar éxito y reanudarlo tras cerrar, conservando el hilo anterior. Comprobar el protocolo con una sesión aislada: autenticación ChatGPT, una respuesta final, reutilización del hilo, error visible al faltar sesión o fallar resume, cierre del hijo propio y ausencia de audio en SQLite y persistencia solo de mensajes finalizados en `presence_history`. Una compilación y un mock no demuestran acceso real a la cuenta ni dictado físico. Las pruebas reales que consuman suscripción deben estar acotadas y anunciadas; las campañas largas requieren aprobación previa. Ejecutarlas con salida de red hacia ChatGPT; en un sandbox sin esa salida, `workspace routing discovery failed` diagnostica el entorno de prueba y no el flujo de Gennie. La sonda opcional `cargo test --locked -p gennie-llm real_app_server_accepts_web_search_request -- --ignored --nocapture` comprueba que el app-server responde a una petición web; no garantiza que el modelo haya buscado.

Narración progresiva: enviar una frase dividida en varios `delta`, confirmar que no se reproduce antes de cerrarla, que se encola una sola vez al recibir puntuación seguida de espacio, y que la última frase se encola al finalizar el mensaje. Si la cola TTS está llena, conservar las frases en orden sin bloquear la UI; al interrumpir por dictado, descartar las frases todavía no enviadas y terminar solo la reproducción propia. Confirmar por separado audio audible con Kokoro en una instancia aislada.


## Historial por conversación

Nuevo chat: verificar que solicitudes repetidas reutilizan el hilo activo vacío sin conectar con el proveedor, también sin sesión en memoria. El diario debe distinguir chat vacío visible de uno con mensajes de cualquier emisor, eliminado, ausente o de otro proveedor; consultar otro chat y renombrar el vacío no alteran esta condición. Los errores de consulta deben propagarse.

Filas Flutter: verificar fechas por días locales (incluidos medianoche, cambio de mes/año y datos inválidos), Hoy/Ayer/Antier, días 3–6 y fecha corta desde 7 días. Probar título y vista previa largos con saltos/Unicode al ancho mínimo, altura fija y elipsis; menú por clic derecho/Shift+F10 sin seleccionar, renombrado con error y borrador conservado. En SQLite, el último mensaje puede ser de usuario, asistente o sistema, pertenece a su propio chat, es opcional y se limita solo para la vista previa; leer el catálogo conserva el activo y el mensaje completo.

Para D-024, probar con DB propia migración desde mensajes anteriores (IDs/texto/fechas intactos y sin vínculo inventado), FK obligatoria, catálogo vacío en instalación nueva, aislamiento entre chats, título del primer usuario con Unicode, reapertura del chat activo, consulta de otro chat sin activarlo, errores como sistema y ausencia de aviso de commit al fallar. En llm, un fallo del diario durante la activación debe hacer rollback del catálogo y conservar el ID previo de proveedor. Los mensajes del usuario se guardan antes del turno y los finales del asistente antes del evento; conservar el borrador si falla el guardado.

## Evidencia de la migración

Verificación completada durante esta migración: tests Rust del workspace, compilación de biblioteca y helper con `scripts/dev.sh --build-only`, análisis Flutter sin incidencias y once tests de widgets Flutter. Se verificó el override `CARGO_TARGET_DIR=/tmp/gennie-flutter-target`. El host Swift pasó comprobación de tipos con un registrador de plugins de prueba; esto no compila ni ejecuta el runner Flutter real. El build macOS release y la firma local se verificaron posteriormente con Xcode 27.1; la interacción física sigue pendiente.

La comprobación real del puente con SQLite temporal pasó: stream, consulta, nombre, error de validación, movimiento reducido y reapertura sin pérdida de mensajes. La suite de widgets incluye una regresión del cierre: comenzar la cancelación del stream, cerrar Rust y después esperar esa cancelación; esperar primero el stream bloqueaba Salir. Resultado final: 88 tests Rust correctos (6 pruebas externas omitidas), 11 tests de widgets, 1 prueba real FRB, Clippy con `-D warnings` y análisis Dart sin incidencias. Las nueve migraciones de presence se compararon byte a byte con el estado previo y permanecen intactas.

## Presentación del lector Flutter

`chat_message_test.dart` comprueba días locales, fechas relativas y largas en español, año distinto, hora HH:mm y ausencia de fechas inválidas. Comprueba texto sin rótulos de emisor y hora debajo a la derecha al ancho mínimo. `widget_test.dart` verifica un badge por cambio de día, acceso a Configuración desde los puntos en claro/oscuro y ausencia de overflow en escritorio y ventana mínima. Con `--dart-define=CAPTURE_UI=true` guarda capturas software con fixtures en `.verification/flutter-ui/chat-{light,dark}.png`; no prueban la composición nativa ni una conversación con el proveedor.

Cabecera de sidebar: comprobar alineación y altura de 56 px respecto al chat, título y acción centrados, sin overflow a 1100 × 720 ni 640 × 420. Los controles AppKit necesitan comprobar por separado centro vertical y área interactiva; las capturas Flutter no incluyen esos controles. Una sonda de ventana oculta verificó centros a 28 pt y hit testing de los tres controles en ambos tamaños; no equivale a interacción física en Gennie. El cambio del runner requiere recompilación.

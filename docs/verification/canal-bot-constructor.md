# Canal del bot al constructor

El bot usa `plugin_build_jobs` con `action=message` para guardar respuestas y correcciones de la misma conversación, arquitecto, época e hilo que originaron el trabajo. La clave de solicitud evita duplicados. Si el constructor espera al usuario, el mensaje reencola el trabajo; si está activo, el host intenta introducirlo en el turno mediante `turn/steer` y el identificador del turno activo. Si el proveedor rechaza el envío, queda pendiente para el siguiente turno. Durante el envío se frenan nuevas llamadas a herramientas y la promoción espera a que no queden mensajes pendientes. Una operación ya iniciada puede terminar.

La bandeja distingue pendiente, en envío, aceptado o incluido en un turno completado, e incierto. La aceptación del proveedor no confirma que el constructor haya aplicado la corrección. Si se pierde la confirmación, el trabajo se interrumpe sin reenvío automático. En Plugins la persona puede revisar el texto, indicar que fue recibido o elegir reenviarlo al reanudar; la revisión no ejecuta la instrucción ni declara que se haya aplicado.

## Evidencia

- Las pruebas locales de la bandeja cubren idempotencia, aislamiento de sesión, reencolado, revisión humana y recuperación de recepción incierta.
- Una prueba de protocolo con un proceso stdio simulado verifica `turn/steer`, el ID del turno y su confirmación.
- `cargo test -p gennie-plugins --locked real_builder_receives_bot_correction_in_active_turn -- --ignored --nocapture`: Codex real, MCP temporal y datos sintéticos; la llamada del bot se simuló mediante la herramienta MCP real. El constructor registró `corregido` tras recibir la corrección durante el turno activo; duración observada 14,83 s. No es una prueba de la interfaz de chat ni de la mediación completa de un bot real.
- `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings`: satisfactorios; se volvieron a probar los paquetes llm y plugins tras el ajuste final del guard.
- `flutter analyze --no-pub lib/src/plugins.dart` y las 40 pruebas existentes de Plugins pasaron; una prueba adicional comprueba que el diálogo exige una elección explícita y no reanuda por sí solo. No equivalen a una prueba visual nativa ni a ejecutar la aplicación compilada.

El código sigue local; no se reinició ni desplegó la aplicación del usuario.

# Probador MCP y revisión nativos, sin inferencia

29 de octubre de 2026. La preparación .verification/native-mcp-probe/1da742f0-c0e2-493d-a1a1-da88277baa24 ejecutó dos fases sobre la misma SQLite ficticia y el release habitual en ventana normal de 1100 × 720. Solo cambió la identidad del bundle copiado para dirigir Computer Use al PID propio. No se forzó tema, escala o preferencias de macOS. Fuente y firma del paquete, datos/contadores y archivos de las fases permanecen en ese directorio.

Plugins, Presence y Assignments activos; conversación/modelos, navegador, guía, audio y Recordatorios apagados. Un servidor Python stdio de prueba devuelve echo; su único efecto es añadir una línea sin argumentos a un contador privado. La entrada desconectar registra ese efecto y termina sin respuesta para simular pérdida del resultado. No opera servicios externos. Programa y contador se copiaron al temporal de la propia prueba para que la app no necesite acceder a Documentos. El perfil MCP usa un ID aleatorio comprobado inexistente antes del ensayo; no enlaza auth.json.

## Fallo de teclado y arreglo

McpAttemptReviewDialog no tenía ruta de DismissIntent con barrierDismissible=false. Dos regresiones macOS reprodujeron que Escape no cerraba la revisión libre ni volvía a permitir cancelar tras un error de guardar. La primera invocación del test desde la raíz falló por falta de pubspec y se conserva separada; red-valid-directory.log contiene las dos regresiones funcionales reales.

Se añaden Actions y FocusScope alrededor del Dialog existente: Escape cancela únicamente si saving=false. PopScope y controles conservan el bloqueo durante guardado; tras error vuelve a aceptar cancelación. El foco regresa al invocador en la prueba de teclado. No cambia campos, selección inicial, tamaño, padding, colores, material o barra. Es el mismo mecanismo de interacción ya aplicado al perfil y a revisiones de Encargos, sin sustituir el marco visual propio de MCP.

Diecinueve casos afectados pasaron. Suite Flutter final: 376 correctas/49 optativas omitidas; análisis limpio, release/firma correctos. Seis capturas de revisión seleccionada en claro/oscuro, ventana normal/mínima y escala normal/150 % son idénticas en RGBA con fuentes/plataforma/fixtures iguales. La comprobación de bloqueo durante IO y recuperación del error usa un Completer de prueba, no latencia artificial de un servicio real.

## Fase de llamadas

PID 6884. Descubrir pobló el catálogo de echo sin ejecutar. Habilitar fuente, elegir permiso y guardar dejaron el contador en cero. Elegir echo presentó el campo Mensaje; un envío vacío mostró «Completa Mensaje.» y conservó foco sin IO. La entrada uno produjo exactamente una llamada con respuesta confirmada y resultado visible. Después desconectar registró el segundo efecto privado y cortó el transporte: el diario quedó uncertain y la interfaz mostró Sin confirmar/Resultado pendiente. Campo, JSON avanzado y Ejecución quedaron deshabilitados; pulsar el botón deshabilitado no añadió una llamada.

Salir se pidió unos 124 segundos después de vincular Computer Use. El proceso terminó con código 0; SQLite íntegro, FK vacías, dos mensajes, perfil Gennie intacto, revisiones 1/1 de Encargos, cero trabajos/ejecuciones/hilos conversacionales. Dos intentos: completed y uncertain; contador exactamente dos. Permiso echo conservado, fuente en revisión 3. calls-result.json/calls-calls.jsonl preservan esta fase.

## Reapertura y revisión

PID 11588, misma base y bundle exclusivos, proceso nuevo. El pendiente reapareció con la fuente desconectada; no hubo otro envío. El diálogo abrió ambas opciones sin seleccionar y Guardar revisión deshabilitado. Escape cerró; una lectura ro confirmó que el intento seguía uncertain y el contador seguía en dos. La revisión se reabrió y se eligió explícitamente La acción se aplicó para el efecto ficticio comprobado mediante el contador. Guardar revisión retiró el pendiente e incrementó la fuente a revisión 4 sin llamar otra vez.

Salir se pidió unos 49 segundos después de vincular la segunda instancia y terminó con código 0. Diario final: completed y reviewed/applied. Contador dos; datos del perfil, mensajes, Encargos, permisos y catálogo conservados. Sin trabajos/ejecuciones/hilos de conversación, FK inválidas ni errores observados de motor/canal. review-result.json, escape-without-resolution.json y NATIVE_COMPLETION.json registran los controles. La opción applied fue una acción de la simulación local, no una evaluación humana o una confirmación sobre un servicio externo.

## Limpieza, fuentes y límites

Se retiraron el temporal de ambas fases, preferencias del bundle QA y únicamente el perfil MCP nuevo. No hubo auth.json en ese perfil, turnos de modelo o guía. La instancia Debug PID 49455 permaneció abierta. Se archivaron fuentes antes/después, test y parche; after-sources-canonical.tar.gz retira una entrada duplicada idéntica del archivo de fuentes inicial, que se conserva con su addendum. La copia utilizada para capturas se retiró. Solo mcp_plugins.dart difiere entre las fuentes habituales de UI anteriores y posteriores. El sello de la campaña de modelos 1e46eae9 sigue coincidiendo; la autorización continúa pendiente.

Este recorrido nativo usa transportes y SQLite reales con efectos ficticios. No acredita cuentas/servicios externos, comparación semántica, cancelación remota o VoiceOver. La ventana mínima, texto ampliado, claro/oscuro y contraste conservan evidencia software y las copias QA de otros recorridos; no se infiere una matriz nativa completa de esta ventana normal. No cambian estilo general/barra, API, SQL o dependencias. El goal sigue activo; sin commit/push, instalación o despliegue.

## Seguimiento al mínimo, oscuro y texto ampliado

La preparación `.verification/native-mcp-scaled/1d95dfa5-ffa7-44e7-ba4c-b3c31d2f8b53` compila una copia QA de las 97 fuentes/manifiestos actuales. Solo cambia main.dart (640 × 420, escala 1,5, highContrast/disableAnimations y ThemeMode.dark) y la apariencia darkAqua de esa ventana Swift. Backend idéntico por hash, firma verificada y producto habitual sin cambios. Las capturas y árboles de Computer Use pertenecen al renderer macOS real; estas condiciones se fuerzan en la copia, sin cambiar preferencias del sistema.

PID 23599, `NATIVE_COMPLETION-v2.json`: pendiente ficticio precargado. Título y acciones permanecen fijos, ambas opciones se leen desplazando el cuerpo, sin selección inicial y Guardar deshabilitado. Escape conserva el pendiente y cero llamadas según `ESCAPE-v2.json`. Reabrir, seleccionar explícitamente not_applied y guardar registra solo esa comprobación ficticia: revisión 2, cero llamadas y Salir 0 en 77,262 s. No acredita un efecto externo no aplicado.

PID 28185, `NATIVE_COMPLETION-probe-exact.json`: otra base, sin pendiente inicial y con contrato precargado extraído literalmente del programa stdio. Campo/ejecución accesibles mediante scroll; una llamada desconectar registra un efecto privado y termina sin respuesta. Progreso, result_uncertain, campo/JSON/envío bloqueados y aviso final visibles mediante scroll. Pulsar el envío bloqueado no añadió llamada. Abrir revisión desde el resultado y Escape conserva uncertain sin resolver. Salir 0 en 62,508 s, contador uno y fuente en revisión 1. Son transportes/diario reales con efecto ficticio, separados del pendiente precargado del recorrido anterior.

Se conservan dos fallos de preparación: el primero consultó la tabla codex_conversation antes de que el arranque la creara y no abrió la app. En PID 26288 el catálogo precargado difería del servidor: la conexión revocó el permiso y devolvió revision_conflict/executed=false, sin intento/efecto; Salir 0 en 67,775 s. Ese recorrido mostró el rechazo vacío junto al campo sin IO, pero no pasa el gate de pérdida de respuesta. No se relajó dicho gate: el ensayo posterior usó el contrato exacto y otra base.

Los tres procesos abiertos terminaron por su menú propio, con integridad/FK correctas, dos mensajes, perfil y Encargos conservados y cero trabajos/ejecuciones/hilos de conversación. Fixtures, perfiles MCP UUID sin auth.json y preferencias propias retirados. Fuentes QA archivadas y copia/cachés retiradas; Debug 49455 siguió vivo. `CLOSURE.json` verifica fuentes y release habituales intactos y sello de campaña de modelos coincidente; cero turnos/guía. No se repiten suites Flutter/Rust porque no cambia código del producto; las cifras anteriores conservan su fecha/ámbito.

Este seguimiento cubre esos estados mínimos en oscuro. No acredita VoiceOver, modificadores físicos, eventos de preferencias del sistema, ratio de contraste nativo ni todos los formularios/temas. Las etiquetas de campos observadas por AX no sustituyen una prueba con lector de pantalla. Goal activo; campaña nueva de modelos aún pendiente de autorización, sin commit/push, instalación o despliegue.

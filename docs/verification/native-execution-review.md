# Actividad, revisión y pausa nativas sin ejecutor

29 de octubre de 2026. Preparación `.verification/native-execution-review/4cb3a9b8-b45f-41d7-978e-be219e9f9355`. Release habitual 1100 × 720, sin cambios de producto, estilo/barra o preferencias. Solo Presence/Assignments activos en SQLite ficticia: Plugins, conversación, guía y demás efectos apagados. Cuatro encargos, ocho políticas y cuatro intentos uncertain precargados; las preparaciones se fechan en 2099, sin Runner. No son ejecuciones producidas por esta prueba.

## Primer recorrido y fallo conservado

PID 77878. La ficha explicó que permite actividad/pausa pero exige Plugins para ejecutar. Actividad de entrega mostró un intento por revisar; el diálogo conservó función/revisión/alcance y parámetros originales. Ambas opciones comenzaron vacías y Guardar deshabilitado. Escape cerró sin resolver, confirmado por lectura ro con cuatro intentos. Reabrir y elegir not_applied dejó Entrega pausada; pausar Preparación retiró próxima ejecución. Su actividad vacía se mostró sin lanzar nada. Salir terminó con código 0, integridad/FK y motor correctos y fixture retirado.

El gate final falló al comparar el hash de bytes JSON de la nueva versión de pausa con la versión precargada por Python. Store vuelve a serializar Policy/Document tipados: orden/espacios y campos opcionales/default difieren de la representación del fixture. run-failed-raw-hash, ESCAPE-failed.json y los drivers iniciales se conservan; ese recorrido no se declara aprobado por el cierre limpio. No guardaba el documento nuevo completo para contrastarlo retrospectivamente.

## Repetición con comprobación de valores

Otro fixture explicita los campos opcionales/default del esquema vigente. El nuevo registro conserva documentos JSON completos y hashes canónicos con claves ordenadas, además de comparar estrictamente políticas, intentos y versiones; no elimina campos/valores de la comparación. PID 84248 repitió abrir actividad/revisión sin opción inicial, seleccionar explícitamente not_applied para el efecto inexistente del fixture y pausar Preparación. Ambas políticas quedaron Pausadas/Sin próxima ejecución. No repite la cancelación/actividad vacía del primer recorrido.

Una lectura ro antes de Salir pasó el mismo guard final: cuatro documentos de encargo y ocho versiones originales iguales; la única versión nueva preparation-1:2 tiene los mismos valores que preparation-1:1. Solo delivery-1 cambia a paused conservando revision=1/disabled; preparation-1 pasa a revision=2/disabled/paused/next_at=null. Run 1 conserva uncertain y añade not_applied; los otros tres intentos y las seis políticas ajenas permanecen iguales. No se creó otra ejecución, trabajo o hilo.

Salir por menú propio terminó con 0, antes de recoger el hijo. run/manifest.json, CANONICAL_READONLY.json y CLOSURE.json conservan el guard aprobado; los cuatro intentos son los precargados. Fixture y preferencias propias retirados; Debug 49455 conservado. Fuente/release intactos, drivers antes/después archivados y sello de campaña de modelos coincidente. No se repiten suites/build por no cambiar código runtime.

## Límites

Son operaciones UI/FRB/SQLite reales sobre estados ficticios; not_applied es una comprobación de la simulación, no valoración humana de calidad o efecto externo. La ejecución real del temporizador/MCP tiene evidencia separada de D-031. Este recorrido no acredita VoiceOver, teclado físico, contraste nativo cuantitativo o matriz mínima/ampliada. Capturas y árboles están en la traza Computer Use. Cero cuenta/modelos/guía, goal activo y autorizaciones pendientes, sin commit/push, instalación o despliegue.

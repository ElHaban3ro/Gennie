# Arquitectos: rechazo nativo con conversación apagada

## Seguimiento: guardado local y reapertura verificados

El 29 de octubre, con la autorización continuada de la sesión, la copia del release `2342cb4a61b4af17f62f025fe0904f5f7ff797e0699f0cfd27ddfeed73d5462c` verificó guardar Bot, Helper y Guía desde la ventana nativa y cerrar/reabrir Configuración. Los tres textos se observaron en las capturas de la reapertura y en SQLite, con una única revisión adicional por perfil. Las demás tablas permanecieron exactamente iguales; Guía siguió apagada. No se modificó código, estilo o navegación.

La preparación [d328ac65](../../.verification/native-architect-save/d328ac65-ac4f-44d2-b26f-a73101292b53/PLAN.json) usa SQLite ficticia, bundle/nombre exclusivos y sandbox de macOS que impide leer/escribir el perfil personal de Codex antes de crear una sesión. LLM está activo y su worker procesa el guardado local; Connect muestra el error esperado del sandbox. Esto acredita guardar con worker desconectado, sin cuenta, turnos o guía; no acredita conversación autenticada. No hay bypass de producción.

La entrada automatizada exigió comprobación visual: `setValue`, selección y pegado podían cambiar el valor AX sin cambiar el texto renderizado. Elevar la ventana y pulsar por coordenadas permitió escribir. No se aisló qué factor resuelve el foco. Además, `typeText` omitió la í de Guía: antes de guardar se registró una [enmienda del literal de prueba](../../.verification/native-architect-save/d328ac65-ac4f-44d2-b26f-a73101292b53/INPUT_AMENDMENT_BEFORE_GUIDE_SAVE.json) a «Prueba local de la guia.». El guard original se conserva fallido, driver 1; el guard independiente con esa única enmienda pasa. No se presenta como prueba de entrada Unicode.

Salir por el menú del bundle propio terminó el producto con código 0; SQLite íntegra, sin FK ni errores conocidos de motor, fixture/preferencias retirados. [Cierre y límites](../../.verification/native-architect-save/d328ac65-ac4f-44d2-b26f-a73101292b53/CLOSURE.json). Reabrir significa aquí reabrir el diálogo en el mismo proceso. Los intentos anteriores [76e51d76](../../.verification/native-architect-save/76e51d76-9b36-4b3f-9a0f-07b3bc1b61df/CLOSURE.json) y [c3a2b238](../../.verification/native-architect-save/c3a2b238-d1d4-4344-a4af-25785e98bc94/CLOSURE.json) siguen incompletos, con sus cierres por límite y discrepancias de entrada; no se sustituyen por este resultado.

Lo siguiente conserva el recorrido histórico con LLM apagado y sus límites de entonces.

29 de octubre de 2026. Preparación `.verification/native-architect-error/12b05607-f79c-449f-b145-d6ec4b7fbd51`. Release habitual 1100 × 720, SHA-256 de origen 074eec394111fac8b38d4abd330acc21d7db4efb3cecc0479e92a257c73b0660. Paquete copiado y firmado con bundle exclusivo, SQLite y contenido ficticios; sin cuenta, modelos, guía o cambio de preferencias del sistema. No cambia código del producto, estilo o barra; no se repiten suites/build por esta prueba de runtime.

## Alcance elegido desde el código

Backend::architect_request permite listar perfiles guardados con LLM apagado; Save exige su Controller y devuelve Conversación desactivada si falta. El worker procesa SaveArchitect antes de crear sesión. Sin embargo, Backend::open del release solicita Connect al activar el módulo: no se puede convertir ese arranque en la prueba sin cuenta/modelos prometida solo porque el guardado posterior sea local. El test open_config de Rust evita esa conexión de arranque y conserva su evidencia propia de guardado con worker real; no acredita guardar desde esta UI. No se añadió un bypass ni se activó LLM para sortear el límite. El guardado satisfactorio nativo/reapertura persistida sigue pendiente.

## Recorrido observado

PID 62482. Configuración mostró selector Bot/Helper/Guía. Helper recibió únicamente el borrador Personalidad = Borrador ficticio, sin tocar habilitación, permisos o modelo. Guardar helper mostró Conversación desactivada junto a las acciones, retiró el estado ocupado y mantuvo el borrador visible. Se cambió a Guía: siguió apagada, con explicación de consultas remotas bajo demanda y consentimiento separado por Encargo. Volver a Helper conservó el borrador propio y no lo confundió con el de Guía.

Cerrar configuración descartó el borrador sin persistir. Reabrir y seleccionar Helper mostró Personalidad vacía, como en SQLite; se cerró otra vez sin guardar. Salir se pidió mediante el menú propio y el proceso ya estaba terminado antes de recogerlo, código 0. Árboles y capturas permanecen en la traza de Computer Use.

## Resultado y límites

run/manifest.json compara los tres perfiles completos observados: identidad/rol/revisión/habilitación/modelo/personalidad/instrucciones y cuatro permisos permanecen iguales. Dos mensajes, revisiones de Encargos 1/1, cero trabajos/ejecuciones, SQLite íntegro/FK vacías y sin los errores conocidos de motor/canal. Fixture y preferencias del bundle QA retirados, instancia Debug 49455 conservada. Fuentes owning del frontend, runtime/worker y driver archivadas, release intacto y sello de campaña de modelos coincidente.

CLOSURE.json distingue explícitamente successful_save_verified=false del rechazo correctamente observado. No se infiere VoiceOver, teclado físico, contraste nativo cuantitativo o matriz mínima/ampliada desde esta ventana normal. No hubo turnos/guía ni nueva autorización consumida. El goal sigue activo; modelos y ajustes reales de accesibilidad continúan pendientes de autorización, sin commit/push, instalación o despliegue.

## Límite de aislamiento contrastado después del editor stdio

La revisión estática de Session::spawn/gennie_directory/prepare_codex_home confirma que el release conversacional usa HOME/Library/Application Support/soc/codex y puede conservar cli-home/auth.json existente. CODEX_HOME solo elige el origen de credenciales cuando falta ese enlace; un CODEX_HOME vacío no aísla por sí solo ese destino preexistente. No se inspeccionaron credenciales ni se abrió esa ruta con un proceso nuevo. SaveArchitect continúa serializado antes de crear sesión, pero Backend::open encola Connect al iniciar LLM. Por tanto no se convierte el guardado satisfactorio nativo pendiente en una prueba sin cuenta solo cambiando CODEX_HOME, ni se añade un bypass de prueba a producción. La evidencia actual de rechazo y del test open_config permanece separada; falta una ruta de ejecución aislada autorizada para el caso satisfactorio.

## Seguimiento: Bot avanzado oscuro al mínimo y texto ampliado

30 de octubre de 2026. Se compiló una copia del frontend vigente, release de origen `4bbc2c927e0718129602386b7118b949f403d4c85dce9c9f8af8f7da3e0c98a3`, con solo tres condiciones QA en main.dart: ventana inicial 640 × 420, ThemeMode.dark y TextScaler.linear(1.5). No se forzaron movimiento/contraste ni preferencias del sistema. Backend y runner macOS reales, perfiles SQLite ficticios y LLM apagado; cero modelos/guía, sin cuenta, cambios de producto o despliegue.

La muestra inicial recorrió opciones avanzadas, campo Modelo, selector de esfuerzo, instrucciones y cambio a Helper. Se observó `gpt-6-luna!` inválido, error junto al campo enfocado/seleccionado y acciones visibles; corregirlo y guardar retiró el error local y mostró el rechazo esperado «Conversación desactivada». Este recorrido alcanzó sus 240 segundos durante el cambio de rol: **producto -15 por terminación del driver, ensayo incompleto**, aunque perfiles/datos íntegros y sin residuales. No acredita retorno de borrador ni reapertura en esta condición.

Un segundo plan se fijó después, limitado a Bot avanzado y cierre, con fixture/UUID nuevos y los mismos 240 segundos. Repitió expansión/desplazamiento y validación de Modelo inválido; el foco llevó el campo y su error al viewport, conservando título, Cerrar, Descartar y recargar y Guardar. Se retiró `!` con BackSpace, el identificador válido quedó renderizado y el siguiente guardado retiró el error local, mostrando «Conversación desactivada». No se interpreta ese rechazo como guardado satisfactorio. Cerró Configuración y **Salir por menú terminó producto/driver 0**, sin errores conocidos de canal, FK o procesos residuales.

Los tres perfiles completos permanecieron idénticos, incluidas revisiones, permisos, instrucciones y Guía apagada; dos mensajes, revisiones de Encargos 1/1 y cero trabajos/ejecuciones. Fixtures y preferencias UUID retirados. Se verificaron 87 fuentes/recursos del producto y el ejecutable habitual sin cambios; proyecto/copia QA retirados. No se repitieron suites por una prueba de runtime sin cambio de código.

La entrada automatizada requirió observar cada resultado: el índice AX del campo podía abrir el selector de esfuerzo; click por coordenadas, activación previa y escritura separada funcionaron. `super+a` añadió texto en vez de seleccionar y se corrigió con BackSpace; no prueba Cmd–A físico. Un noWindowsAvailable transitorio se recuperó leyendo el mismo PID vivo, sin reabrir la app. La causa del enrutamiento no se aisló. Las capturas/árboles están en la traza Computer Use; no se infiere VoiceOver, contraste físico cuantitativo o toda la matriz desde esta muestra.

[Resultado y límites](../../.verification/native-architect-advanced/cc619f0b-8774-43d5-ad6d-1e66e4372718/RESULT.json), [preparación](../../.verification/native-architect-advanced/cc619f0b-8774-43d5-ad6d-1e66e4372718/PREPARED.json), [primer fallo conservado](../../.verification/native-architect-advanced/cc619f0b-8774-43d5-ad6d-1e66e4372718/run/manifest.json), [alcance del segundo recorrido](../../.verification/native-architect-advanced/cc619f0b-8774-43d5-ad6d-1e66e4372718/validation-and-quit/PLAN.json) y [cierre correcto](../../.verification/native-architect-advanced/cc619f0b-8774-43d5-ad6d-1e66e4372718/validation-and-quit/run/manifest.json). Quedan Helper/Guía avanzados al mínimo/150 %, entrada Unicode y regreso/reapertura de borradores en esta condición; el guardado normal previo mantiene su alcance propio.


## Seguimiento: Helper/Guía avanzados y borrador al mínimo ampliado

La [auditoría conjunta](assignment-label-audit.md#helper-y-guía-al-mínimo-ampliado) añade consulta avanzada oscura 640 × 420/150 %, Helper/Guía, borrador q exclusivo de Helper conservado al volver de Guía y descartado al cerrar/reabrir el diálogo. Producto/driver Salir 0, perfiles/permiso/historial intactos, cero modelos y recursos retirados. No guardó perfiles en esa condición ni demuestra reinicio, Unicode o VoiceOver; el guardado normal previo conserva su alcance.

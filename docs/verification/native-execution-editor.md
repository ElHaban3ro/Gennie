# Configuración de ejecución: foco y recorrido nativo

29 de octubre de 2026. Preparación `.verification/native-execution-editor/f3272e68-622a-42c6-b265-a7487bb51425`, sobre el checkout actual. No se modifica el estilo general ni la barra lateral. El goal permanece activo; sin commit/push, instalación o despliegue.

## Fallo y arreglo

Tres regresiones de plataforma macOS reprodujeron que el error se veía, pero Retraso máximo y JSON avanzado no recibían foco después de editar otro campo; un horario ausente tampoco permitía abrir el selector con Espacio tras el rechazo. La validación genérica pedía foco al FocusScope del diálogo. Solo los parámetros de función tenían nodos propios.

ExecutionDialog registra ahora nodos y anclas para horario, JSON y retraso, los libera al desmontar y recorre los FormFieldState inválidos en orden visual. Enfoca el control exacto y lo desplaza después del layout. No duplica validadores, modifica datos/contratos, activa políticas ni cambia la superficie. Las tres regresiones y catorce casos afectados pasaron; el caso de retraso también corrige y guarda pausada con borrador conservado. `red.log` y `green.log` permanecen en `.verification/execution-validation-focus`.

## Capturas y verificaciones

Las seis variantes de claro/oscuro, 1100 × 720/mínimo y texto normal/150 % usan plataforma macOS explícita. Al hacerlo, el muestreo anterior del rótulo Guardar caía fuera del botón compacto en texto ampliado. Se conservan los dos rechazos de medida; la prueba mide ahora dentro de su Material y fuera de las letras mediante el helper existente, manteniendo 4,5:1. No cambian colores o geometría para obtener el resultado. Un fallo previo por omitir design/chats_preview.dart en la copia también se conserva como preparación fallida, no regresión del producto.

Doce imágenes RGBA completas de configuración/error son idénticas antes/después con la misma plataforma, fuente y fixtures; dieciocho muestras, mínimo 5,25780:1. Se inspeccionaron las capturas mínima oscura/configuración y mínima clara/error. Estas capturas no contienen los tres rechazos nuevos ni prueban contraste físico. Suite Flutter: 382 correctas/49 optativas omitidas, análisis limpio. Release habitual y copia QA compilaron y pasaron firma local. Sin cambio Rust/API/SQL que justifique repetir sus pruebas.

## Interacción nativa

La copia QA cambia solo main.dart y la apariencia de su ventana Swift para iniciar 640 × 420, ThemeMode.dark/darkAqua, texto al 150 %, alto contraste y movimiento reducido de MediaQuery. Backend idéntico al release por hash. No cambia preferencias del sistema. Plugins, Presence y Assignments activos únicamente en SQLite ficticia; conversación/modelos, guía, navegador, audio y Recordatorios apagados. Fuente MCP ficticia precargada y sin transporte abierto; no se descubre o ejecuta una herramienta.

PID 44603. La ficha permitió abrir Configurar ejecución y leer alcance/zona/función mediante scroll, con título y acciones fijos. Avisos y activación estaban apagados; Guardar pausada vacío mostró Consulta y foco automáticamente. Se escribió novedades ficticias; luego se introdujo retraso cero y se devolvió el foco a Consulta antes de Guardar. El rechazo trajo automáticamente el retraso y su error a la vista, con cero seleccionado/foco. Una lectura ro confirmó cero políticas, ejecuciones, intentos MCP y llamadas tras ambos rechazos.

Se sustituyó cero por diez y se guardó una sola configuración pausada, sin resultados al chat ni guía. La ficha mostró Entrega/Pausada y Sin próxima ejecución. Reabrir conservó consulta y diez; Escape cerró sin otra escritura. Salir por el menú terminó con código 0 en 92,379 s, sin errores observados de motor/canal. Un clic por índice AX no había enfocado el retraso: se comprobó el valor cinco en la imagen y se usaron coordenadas de la captura actual. Los índices/valores de campos AX no se presentan como lectura de VoiceOver.

## Estado final y límites

NATIVE_COMPLETION.json registra SQLite íntegro/FK vacías, una política revisión 1/disabled/paused con argumentos exactos y retraso diez; cero ejecuciones/llamadas/intentos/trabajos/hilos. Los cuatro documentos y sus revisiones, los dos mensajes y el perfil permanecieron iguales. Fixture, perfil MCP UUID sin auth.json y preferencias QA retirados. Fuentes anteriores/posteriores y QA archivadas; copia/cachés retiradas. Debug 49455 siguió vivo, sello de campaña de modelos coincidente y cero turnos/guía.

VoiceOver, modificadores físicos, preferencias reales del sistema y el resto de la matriz permanecen pendientes. Este recorrido observó además un error previo de Consulta que seguía visible hasta otra validación/Guardar. El seguimiento inferior lo reproduce y corrige; esta muestra anterior conserva su versión. No se presenta este recorrido como cierre completo de UX, semántica de modelos o ejecución de horarios. Las autorizaciones nuevas de accesibilidad y modelos siguen pendientes.


## Seguimiento: retirar el error durante edición y enfocar Función

Preparación `.verification/execution-validation-edit/f9edcaea-0f15-497f-acfb-99f4745b7708`. Cuatro casos macOS reprodujeron errores locales que permanecían tras corregir Consulta, retraso, JSON y horario. Se amplían tres pruebas y se añaden dos, para cinco casos finales: otro fixture con varias funciones reprodujo que el selector Función ausente tampoco recibía foco. No se completan datos ni se escoge una función por el usuario.

Los controles validados usan AutovalidateMode.onUserInteraction individual, sin validar de golpe todos los campos intactos; mantiene validadores, borrador y API humana. El selector Función incorpora ancla/nodo propio al recorrido visual y los libera al cerrar. La elección por teclado retira su error sin configurar. Dieciséis casos afectados correctos, suite 384/49 y análisis limpio. Doce capturas macOS saludables de configuración/error global son idénticas en RGBA antes/después, con dieciocho muestras de contraste software y mínimo 5,25780:1. No contienen el cambio deliberado de los errores locales durante edición. Release/firma correctos; sin cambios Rust/API/SQL.

PID 52784, release habitual 1100 × 720 sobre otra base/bundle propios, sin tema/MediaQuery forzados o ajustes del sistema. Guardar vacío mostró Consulta y foco; escribir novedades ficticias retiró el error sin otro Guardar. Escribir retraso cero presentó su error; corregir a diez lo retiró, conservando foco/borrador. Capturas y AX en la traza Computer Use. CORRECTED_BEFORE_SAVE.json confirmó por lectura ro cero políticas, ejecuciones, intentos MCP y llamadas después de corregir.

Una única escritura posterior creó una política pausada, sin avisos/guía/próxima ejecución. Salir por menú terminó con 0 en 60,074 s, sin errores observados de motor/canal. SQLite íntegro/FK vacías; documentos/revisiones de los cuatro encargos, dos mensajes y perfil iguales; cero trabajos/hilos/ejecuciones/llamadas. Fixture/perfil MCP sin auth.json/preferencias propios retirados, fuentes antes/después archivadas y copia de capturas retirada. Debug 49455 conservado y sello de modelos coincidente. Cero turnos o guía; las autorizaciones nuevas siguen pendientes.

El foco/elegir Función y el JSON tienen regresiones de widgets macOS; este recorrido nativo solo acredita Consulta/retraso y guardado pausado. La prueba mínima nativa anterior conserva su versión; las capturas actuales cubren mínimo/150 % en software, sin renovar toda esa matriz física. No acredita VoiceOver, modificadores físicos, preferencias reales del sistema o contraste nativo numérico. Estilo general/barra intactos; goal activo, sin commit/push, instalación o despliegue.

# D-054 — tres sesiones por página y copia histórica vigente

29 de octubre de 2026. Auditoría del requisito original de cargar dos o tres conversaciones, sin traer todas las sesiones del asistente. Cambio local de lectura y verificación; sin SQL/migraciones nuevas, FRB, dependencias, contexto de inferencia o estilo visual nuevo.

## Desajuste y comportamiento

El lector limitaba mensajes y caracteres, pero podía incluir muchas sesiones cortas. La regresión de 300 sesiones mostró diecinueve en una página con presupuesto de 37 mensajes/250 caracteres. Ese límite no cumplía por sí solo el máximo de sesiones pedido.

assistants::page añade un máximo de tres conversation_id distintos, junto con los límites existentes. La API del lector mantiene sesenta mensajes y 24.000 caracteres; sigue entregando íntegro el primer mensaje aunque sea mayor. Antes de leer el cuerpo y metadatos de una cuarta sesión se termina la página. El cursor sigue perteneciendo al último mensaje entregado: el siguiente de esa cuarta sesión queda para la consulta posterior, sin saltos ni duplicados.

Se cuentan sesiones distintas, no bloques consecutivos. Un aviso tardío de una sesión ya incluida no consume otra plaza. Una página puede contener cuatro mensajes de tres sesiones intercaladas; no se trunca a tres mensajes. La paginación no modifica la selección, títulos, historial o hilo del proveedor. El límite corresponde a cada consulta del lector, no es una promesa de que toda la aplicación conserve solo tres sesiones en memoria.

## Evidencia actual

Las dos regresiones fallaron antes: 300 sesiones cortas excedían tres, y el caso intercalado devolvía todo sin cortar ante una cuarta sesión. Después se recorren los 300 mensajes originales sin repetir una inserción posterior al cursor, con máximo de tres sesiones por página y sesión activa intacta. La prueba intercalada conserva los siete mensajes en dos páginas. Las pruebas previas de una sesión larga, Unicode, citas y mensaje individual mayor que el presupuesto siguen correctas.

Suite Rust: 405 correctas, 27 optativas omitidas; Clippy workspace/all-targets, formato e invariantes correctos. Biblioteca/helper debug y paquete release compilados por los lanzadores y firma ad hoc verificada. El puente Flutter/FRB/Rust/SQLite pasó seis recorridos sobre datos ficticios, incluidas cuatro composiciones de lector y retirada del original citado con fragmento conservado. Su comprobación de versión retenía 0015: ahora exige la versión máxima de las migraciones disponibles, actualmente 0019. No se presenta ese flujo software como interacción nativa nueva.

La copia histórica D-040 había llegado a Presence 0018 y aún no cubría 0019/Browser 0001. Se revalidó el origen con mode=ro/query_only y se ensayaron copias privadas con todos los módulos apagados. El reporte 21f6c084-58f7-4a8b-99de-cf68c35c6022 comprobó las migraciones actuales antes del nuevo límite; 1a7da985-4f66-4945-8820-adb3fb1aec50 verificó después la paginación más corta. El reporte final a05a272d-1bf8-40e2-9ea7-91d94d9eee3f añade recuento/assert de sesiones en cada página.

En el ensayo final se conservaron 57 sesiones, 1.046 mensajes, un encargo y 26 trabajos, sin cambiar las columnas originales de las tablas existentes. Los 1.046 mensajes visibles se recorrieron en 22 páginas sin duplicados, cada una con hasta tres sesiones. Primera página: sesenta mensajes, 8.224 caracteres y tres sesiones, lectura observada de 8,163 ms. Es una observación, no una comparación de rendimiento. Rollback por interrupción durante la transacción 0012, restauración de copia, reapertura, integridad/FK y asociaciones comprobados. Presence terminó en 0019 y Browser en 0001; cero módulos de efectos iniciados, políticas de ejecución nuevas o llamadas a modelos. Todas las copias crudas se retiraron. No se migró la base del usuario ni se reinició su instancia Debug.

Logs session-budget-* y session-budget-finish.json en .verification/assistant-chat-real; reportes privados en .verification/migration-audit. La suite Flutter habitual conserva el cierre de D-053 (359 correctas/49 optativas omitidas); no se repitió sin cambio Dart. No se modificó ninguna migración previa, permiso, estilo general o barra lateral. Sin commit/push, instalación o despliegue.

## Investigación pendiente

La prueba breve autorizada 187c5232-783e-4540-82d2-27012918e111 cerró antes de este cambio: un turno y cero guía, lote rechazado y detención de las cuatro filas restantes. Los siete metadatos estaban presentes en ambos candidatos, pero el segundo tenía document=null y questions=[] al proponer retirar un duplicado. No se guardó un borrador válido; la comparación sigue inconclusa y no se repite bajo ese permiso. Los tres turnos máximos no ejecutados no son autorización para otro contrato/campaña. Fuentes antiguas, uso reportado y fallo se conservan en su archivo sellado; una evaluación posterior debe preparar su propio alcance.

La [auditoría vigente](../verification/plan-audit.md) consolida los avances posteriores de procedencia/mediación. Continúan comprensión con proveedor activo, guía y calidad/revisión humana y matriz nativa/VoiceOver. Estos pendientes no se sustituyen por el éxito de paginación o de la copia.


## Seguimiento nativo con cientos de sesiones

El [recorrido ea626a4b](../verification/native-large-history.md) ejecuta el release habitual sobre 300 sesiones/600 mensajes ficticios. Tramo reciente, carga de anteriores y cita desde sesión 300 al primer mensaje; foco/Menu→Responder/cancelar, sesión activa 300 e historial/cita íntegros, cero hilos/modelos y Salir 0. No instrumenta el número de consultas o renueva el benchmark de Rust; distingue el presupuesto por página ya probado de la interacción del renderer. Código/estilo/barra y paquete sin cambios; fixture retirado y goal activo.

# D-031 — Ejecución local explícita de Encargos

29 de octubre de 2026. Continuación parcial de D-025, D-028 y D-030. La ficha permite configurar una función concreta, revisar parámetros y activar su calendario; registrar o aceptar una propuesta no activa nada. Mantiene el estilo general y la navegación lateral. El goal de chats/agentes sigue activo.

## Propiedad y permisos

Assignments posee calendario, políticas y diario de intentos. Bootstrap compone un único Runner con un adaptador al Controller existente de Plugins; no añade otra instancia de navegador o cliente MCP, nuevo AppModule, HTTP, bus o ejecutor del sistema operativo. Los plugins siguen utilizables sin Encargos. No hay inferencia para calcular horarios o ejecutar una función configurada.

La API humana execution_request admite consulta, configuración, pausa, actividad paginada, detalle de un intento y comprobación de resultado. No se publica una herramienta MCP para activar/reanudar/reconciliar. Se fijan ID, instance_id y revisión del encargo, documento completo, función, revisión del plugin, parámetros objeto JSON, vía y política de retraso/vencimiento. Solo se configura un encargo activo con método y autorización explícitos; guardar pausada también valida estos datos. Cada instancia admite una política de entrega y otra de preparación. El control de revisión impide sobrescribir cambios concurrentes. No se eligen destinatarios ni parámetros obligatorios por defecto.

La preparación de Plugins comprueba revisión, contrato, permisos y resultados pendientes sin ejecutar ni abrir transporte. Para web exige verificación funcional de esa revisión; para MCP verifica esquema y permiso, sin confundir disponibilidad con verificación de efectos. Antes de cada envío se repiten estas comprobaciones y las del documento. Un cambio del encargo o de permisos/función detiene esa configuración; no se actualiza silenciosamente para seguir actuando. Los parámetros/cuerpos quedan en SQLite local con los mismos permisos de la base, no en logs ni en prompts de la guía.

## Calendario y actividad

Se usa el horario existente: notify_at puntual o recurrence diaria/semanal con zona IANA para entrega; preparation.interval_minutes para preparación, anclado al instante de configuración y calculado en UTC. No se inventa un segundo horario. Jiff usa los archivos de zona del sistema; ver dependencias. Para una hora repetida se elige explícitamente la primera o segunda ocurrencia, una sola vez. Para una hora inexistente se omite o desplaza hacia adelante. UTC se persiste canónicamente; la UI distingue la zona del encargo de las fechas mostradas en hora del equipo.

El worker serial espera hasta la siguiente ocurrencia con comprobación de reloj cada sesenta segundos como máximo mientras haya calendario activo. No mantiene lock SQLite durante IO. En reposo/reinicio, skip_missed considera el pendiente guardado y omite lo que exceda el retraso permitido; run_latest toma solo el pendiente más reciente. Ambos avanzan al futuro sin reproducir una cola histórica. Retraso máximo de 1..1440 minutos, vencimiento opcional y validación antes de enviar. Un retroceso del reloj no repite la misma ocurrencia.

Solo opera mientras Gennie siga abierto y el equipo pueda ejecutar procesos. Cerrar la ventana mantiene los servicios existentes; Salir los termina. No despierta macOS ni ejecuta con la aplicación cerrada. El bloqueo exclusivo de archivo junto a SQLite impide dos Runner sobre esa base. El despacho de funciones requiere Assignments y Plugins activos. D-032 también mantiene un Runner con Assignments y conversación activos aunque Plugins esté apagado: solo entrega resultados pendientes al bot, sin despachar funciones ni avanzar calendarios. Si faltan Plugins y conversación, queda Controller::view para consultar, pausar y revisar. No activa módulos en caliente ni altera configuraciones existentes al migrar.

## Persistencia, incertidumbre y cierre

0006 añade políticas, versiones inmutables y ejecuciones al namespace histórico events; conserva 0001..0005 y todos los planes. Las filas de ejecución sobreviven a borrar un encargo; un número reutilizado no hereda sus políticas. UNIQUE(policy_id, scheduled_at) evita duplicar una ocurrencia incluso tras cambiar revisión. Primero se reserva y avanza calendario en transacción; luego se marca running antes del efecto. Nunca se vuelve a enviar automáticamente ese intento.

La función que informa éxito produce succeeded; no equivale a prueba universal del estado externo. Rechazo conocido sin efecto pausa; error o respuesta ambigua produce uncertain y bloquea futuras ocurrencias hasta comprobación humana. El fallo al guardar una respuesta también fuerza recuperación antes de otro envío: running se vuelve uncertain, reserved se cancela. Tras reiniciar se aplica la misma regla. Cambiar parámetros, función o revisión no elude una incertidumbre pendiente de esa política; otras políticas con la misma función también se bloquean.

La comprobación humana no tiene opción inicial, conserva el documento/función/parámetros originales del intento y deja la política pausada. Reanudar exige volver a configurar. MCP conserva además su diario propietario y bloqueo por herramienta: si existe mcp_attempt_id se enlaza a Plugins para revisar allí también; comprobar Encargos no cierra ese otro recibo. No se afirma idempotencia externa ni equivalencia semántica entre funciones, ni cobertura del uso manual web con efectos inciertos.

Resultados hasta 32 KiB por intento; si exceden el límite se registra omisión. El listado devuelve hasta cincuenta resúmenes por cursor y carga el cuerpo/versión individualmente. Los errores libres del transporte no se persisten. Las revisiones de UI se publican después del commit, incluidos reserva, recuperación y reconciliación. Al salir se marca stop antes de cancelar los transportes propietarios, se drenan las peticiones humanas, une el Runner y cierra SQLite al final. No se deshace un efecto enviado; pausar solo impide los siguientes.

## Evidencia y pendientes

Pruebas con reloj controlado cubren mes/año, Bogotá, cambios DST de Nueva York, salto civil de Apia, coalescencia, retroceso, permisos/documento editado, vencimiento, errores de reserva/confirmación, pausa durante IO, exclusión de instancia y recuperación de archivo SQLite con versión original. Los preflight de web/MCP prueban ausencia de ejecución/transporte/recibo en sus fixtures. Compilación y UI se registran en testing.

La sonda optativa installed_scheduler_mcp_executes_once_and_does_not_repeat_a_lost_response pasó en 5,82 s usando Codex CLI instalado como transporte MCP stdio, servidor Python y contador propios: dos efectos, uno confirmado y otro cuya respuesta se perdió después del cambio, cero reenvíos automáticos. Sin autenticación, inferencia, cuentas, perfil personal ni construcción; fixture e hijos retirados. Una muestra no demuestra semántica de servicios reales.

D-032 continúa la entrega de resultados por el bot con origen durable y D-034 implementa valoración contextual de preparación mediante guía→bot bajo consentimiento separado. Continúan la evaluación semántica amplia, interacción nativa/accesible restante y comparación empírica. Los mecanismos del sistema operativo solo se incorporan si se solicitan; no se promete ejecución con Gennie cerrado. La actividad técnica no se presenta como notificación ya enviada al usuario. La [auditoría del plan](../verification/plan-audit.md) distingue reloj controlado, temporizador real y pruebas físicas aún no acreditadas.

Nueve widgets nuevos verifican parámetros obligatorios, activación apagada, cadencia de preparación sin entrega, revisión concurrente/borrador, pausa por revisión, actividad paginada y comprobación sin elección inicial. Doce capturas de configuración/error cubren seis combinaciones claro/oscuro, ventana amplia/mínima y texto al 150 %; dieciocho mediciones de controles/error, mínimo 5,25780:1. Cuatro flujos adicionales con FRB/dylib/SQLite reales guardaron cuatro pausas y cuatro comprobaciones humanas sobre intentos ficticios preexistentes, sin ejecutar funciones ni modificar planes. Fixture cerrado y retirado; cuatro capturas adicionales. No equivalen a interacción física del runner o VoiceOver.


## Foco de validación y configuración nativa

Tres regresiones macOS reprodujeron foco perdido en retraso/JSON/horario ausente. Se registran nodos/anclas propios y se enfoca el primer FormFieldState inválido en orden visual, sin cambiar validadores o estilo. Catorce afectados y 382 Flutter/49 optativas, análisis/release/firma correctos; doce capturas RGBA iguales y contraste software mínimo 5,25780:1 con muestra correcta del botón. El [recorrido nativo mínimo/oscuro/150 %](../verification/native-execution-editor.md) verificó errores/foco, guardar pausada y reabrir sin activar horarios o avisos, cero llamadas y Salir 0. No acredita VoiceOver ni efectos externos; la retirada de error durante edición se continúa en el seguimiento siguiente.


El [seguimiento de validación durante edición](../verification/native-execution-editor.md#seguimiento-retirar-el-error-durante-edición-y-enfocar-función) retira errores locales al corregir cada control sin guardar ni validar todos los campos intactos. Incorpora también Función al foco de campos inválidos. Cinco casos macOS finales, dieciséis afectados, 384 Flutter/49 optativas y análisis/release/firma correctos; doce capturas saludables iguales. El release normal verificó Consulta/retraso corregidos con cero escrituras previas, una política pausada y Salir 0. No acredita interacción física del selector Función o JSON ni cambia permisos/estilo.


El [recorrido nativo de actividad/revisión/pausa](../verification/native-execution-review.md) verifica consulta y pausa con Plugins/conversación apagados sobre intentos ficticios precargados. Conserva cancelación sin resolver, revisión sin elección inicial, parámetros originales y actividad vacía como observaciones del primer proceso, cuyo guard de bytes falló. La repetición con comparación estricta de valores JSON pasó comprobación+pausa, cuatro intentos sin otros envíos, versiones/documentos conservados y Salir 0. No cambia runtime ni demuestra otro efecto del temporizador o servicio externo.


## Próxima fecha en la lista

La proyección de lectura upcoming_execution_times pertenece a Assignments: máximo cien IDs positivos visibles, unión por instancia, políticas enabled/active, versión y documento activos/vigentes, mínimo por instante persistido. Sin preparar, recomputar calendario o ejecutar; no añade migraciones. Bootstrap traslada el opcional al snapshot/FRB y Flutter lo presenta en hora local como fecha prevista. La [verificación](../verification/assignment-next-execution.md) incluye mínimo/pausa/obsolescencia/identidad/lectura, reapertura y cuatro estados nativos sin Runner; no promete ejecución con el producto cerrado o una función indisponible.

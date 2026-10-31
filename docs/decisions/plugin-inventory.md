# D-041 — Inventario MCP del bot independiente del navegador

29 de octubre de 2026. Corrección local de composición, sin nueva interfaz pública, preferencias, migraciones, FRB o cambios visuales. Continúa Plugins 1/3 y la auditoría de arquitectos del [plan](../plans/gennie-chats-agentes.md).

## Fallo reproducido y cambio

Bootstrap registraba todas las herramientas de Plugins únicamente con plugins, browser y llm activos. Por tanto, una fuente MCP configurada podía consultarse/probarse manualmente después de D-039, pero el bot no recibía plugin_mcp_catalog/plugin_mcp_run si Gennie Web estaba apagado. El transporte MCP y sus permisos no necesitan ese navegador.

El registro ahora requiere plugins y llm; sin browser publica solo esos dos proxies MCP. Con browser conserva el conjunto anterior, incluido el constructor. Las herramientas web/de construcción siguen condicionadas a browser. No habilita módulos, conecta fuentes, concede permisos ni crea Encargos. Los callbacks y su validación/diario son los mismos; no cambia el comportamiento del constructor o los límites guardados del bot.

## Evidencia

La regresión `bot_mcp_inventory_is_independent_of_browser_and_assignments` falló antes del cambio por ausencia de ambos proxies. Después recorre las cuatro combinaciones de llm/plugins activos/inactivos, con browser y assignments apagados. Consulta tools/list por el socket Unix real del host, verifica que solo la combinación permitida publica ambos proxies y que no aparecen herramientas web, constructor o Encargos.

En esa combinación prepara una fuente/catálogo sintéticos guardados y llama al proxy real de catálogo por el socket. La ejecución con campo obligatorio ausente devuelve invalid_arguments/executed=false antes de abrir el cliente externo. No descubre ni consulta la URL ficticia. Los contadores permanecen en cero para Encargos, intentos, trabajos, hilos de proveedor y mensajes. El host se cierra y retira su socket en las cuatro combinaciones. Son pruebas del registro/controladores, sin cuenta, inferencia, navegador o servicio externo; no prueban decisiones semánticas del modelo.

Cierre automatizado: 366 pruebas Rust correctas y 24 optativas omitidas; Clippy workspace/all-targets con -D warnings, formato, invariantes y diff correctos. Evidencia en `.verification/assistant-chat-real/mcp-inventory-{before,after,workspace,clippy}.log`. No cambia Dart ni la presentación; no se repiten capturas por este cambio de registro.

`scripts/dev.sh --build-only` y `scripts/build.sh` compilaron biblioteca/helper debug y paquete macOS release, con firma ad hoc local verificada. Logs `mcp-inventory-{debug,release}.log`. No se abrió, instaló o desplegó el paquete. La recompilación invalida el manifiesto de binarios del piloto D-038 anterior: antes de una campaña autorizada hay que prepararlo de nuevo con el mismo alcance solicitado; no se inició inferencia.

## Auditoría de comunicación que continúa

La ruta moderna de mensajes del constructor al chat pasa por PluginJournal::deliver→Receiver de llm::delegation→worker del bot→diario de Presence, según D-029/D-033. Los proxies MCP entregan resultados al bot y no escriben directamente mensajes del usuario. Las pruebas de mediación existentes cubren estados rutinarios, redacción, errores/recuperación, origen y deduplicación, con su alcance documentado en testing.

Se identificó una ruta distinta, resuelta posteriormente en [D-042](builder-messages.md): `frontend/lib/src/plugins.dart` muestra directamente job.summary y events[].message en la ficha, «Último paso» y actividad de construcción. `apps/plugins/src/worker.rs` admite texto libre del helper en report(state=progress), que llega a esos campos. La mediación del chat no demuestra que esa presentación cumpla el requisito de que cualquier comunicación del helper al usuario atraviese al bot. Hay que sustituir la comunicación narrativa de esa ficha por mensajes confirmados del bot o estados operativos del host, preservando artefactos, autorización, procedencia y estilo general. Este hallazgo no se declara resuelto por corregir el inventario.

D-042 cierra la mediación narrativa de esa ficha. Siguen abiertos la auditoría integral, contenido MCP nativo/equivalencias, diario web/cancelación, evaluación semántica consentida, comprensión real de citas y matriz física/VoiceOver. La migración histórica y la paginación tienen evidencia acotada en D-040; escala mayor permanece pendiente. Goal activo, sin commit/push, instalación o despliegue.


Estado posterior, 30 de octubre: las [rutas narrativas actuales](../verification/architect-communication.md#revalidación-del-alcance-actual--30-de-octubre-de-2026) tienen auditoría, cotejo de fuentes y pruebas de error/recuperación. Las indicaciones anteriores de auditoría pendiente describen aquel corte. No se añade una garantía semántica universal ni se reabren campañas cerradas.

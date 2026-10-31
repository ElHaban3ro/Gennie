# Cierre funcional de Gennie

Seguimiento posterior: la revisión del usuario encontró una contradicción omitida en las instrucciones del bot sobre el ejecutor de Encargos. El [arreglo y la entrega actualizados](assignment-instructions.md) prevalecen sobre la versión inicial descrita debajo. Las cifras, sellos y paquete de este informe conservan el estado de aquella entrega; no se presentan como nuevas comprobaciones del ajuste posterior.

30 de octubre de 2026. La implementación funcional del plan queda revisada en sus 18 requisitos numerados. El usuario retiró todo trabajo adicional de accesibilidad, contraste y sus matrices; no se reabre ni se usa como condición de cierre. Se conservan el estilo general y la barra lateral.

## Requisitos y evidencia

| ID | Resultado revisado | Fuente y evidencia | Alcance |
|---|---|---|---|
| 1.1 | Identidad única y migración conservadora | [Código](../../apps/presence/migrations/0012_assistant_identity.sql), [evidencia](../../.verification/migration-audit/a05a272d-1bf8-40e2-9ea7-91d94d9eee3f/report.json) | Copia histórica ro,57 sesiones/1046 mensajes, rollback/respaldo/reapertura; no migración del origen. |
| 1.2 | Chats por asistente, nombre/avatar editables | [Código](../../frontend/lib/src/desktop.dart), [evidencia](../../docs/verification/native-profile.md) | Un solo Gennie; no creación de bots adicionales. |
| 1.3 | Nueva sesión y contexto solo activo | [Código](../../apps/llm/src/codex.rs), [evidencia](../../docs/verification/provider-chat-probe.md) | Aislamiento con proveedor real en11c53fcd; no se promete conducta universal. |
| 1.4 | Paginación estable y carga progresiva | [Código](../../apps/presence/src/assistants.rs), [evidencia](../../docs/verification/native-large-history.md) | Copia real22páginas≤3sesiones; primer mensaje grande íntegro; pruebas de ancla/error sin cambiar contexto. |
| 1.5 | Responder, cita y navegación al original | [Código](../../apps/presence/src/replies.rs), [evidencia](../../docs/decisions/quote-navigation-focus.md) | Texto delimitado, original eliminado y carga distante; no se exige ensayo físico adicional de accesibilidad. |
| 1.6 | Evidencia real por respuesta y cancelación | [Código](../../apps/presence/src/tool_evidence.rs), [evidencia](../../docs/decisions/chat-cancellation.md) | Correlación turno/ítem; fragmentos explícitos y sin argumentos privados. No ACK remoto/facturación inferidos. |
| 2.1 | Plugins independientes y origen MCP | [Código](../../apps/plugins/src/external_mcp.rs), [evidencia](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/workspace.log) | Inventario con Browser/Assignments apagados y catálogo sin conectar. |
| 2.2 | Parámetros y aclaración contextual | [Código](../../apps/llm/src/plugin_input_tests.rs), [evidencia](../../.verification/chat-provider/560a7926-5f7d-45f5-9094-ad0cc8831a2b/REPORT.md) | Dato faltante/destinatario ambiguo; efecto ficticio único tras aclarar. |
| 2.3 | Catálogo, permisos, formularios y uso puntual | [Código](../../frontend/lib/src/mcp_plugins.dart), [evidencia](../../.verification/native-mcp-probe/1da742f0-c0e2-493d-a1a1-da88277baa24/CLOSURE.json) | Dos llamadas stdio con contador privado, cero reenvíos; sin cuentas externas. |
| 2.4 | Fallos, permisos e incertidumbre | [Código](../../apps/plugins/src/mcp_attempts.rs), [evidencia](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/workspace.log) | Validación previa, bloqueo de fuente/función y revisión humana; no equivalencia semántica universal. |
| 3.1 | Definición/calendario/intentos/auditoría separados | [Código](../../apps/assignments/src/execution.rs), [evidencia](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/workspace.log) | IDs de instancia y revisiones; migraciones históricas intactas. |
| 3.2 | Creación contextual y resumen antes de activar | [Código](../../frontend/lib/src/assignment_execution.dart), [evidencia](../../.verification/native-assignment-edit/69224db6-e279-4ff8-ac00-b6c4b89f9d87/CLOSURE.json) | Crear/editar/cancelar; políticas inicialmente apagadas; preguntas evaluadas en el corpus, no garantía universal. |
| 3.3 | Propuestas revisables, refinamiento y aplicación humana | [Código](../../apps/assignments/src/proposals.rs), [evidencia](../../docs/verification/assignment-metric-coverage.md) | Diez casos medidos y elección humanaB acotada; sin formato ganador general o aplicación personal. |
| 3.4 | Scheduler explícito, recuperación y entrega única | [Código](../../apps/assignments/src/schedule.rs), [evidencia](../../.verification/assistant-chat-real/scheduler-real-mcp.log) | Reloj real/stdio y2efectos ficticios; reloj controlado paraDST/saltos/caídas; no daemon con app cerrada. |
| 3.5 | Evaluación empírica y métricas de avisos | [Código](../../bootstrap/tests/fixtures/contextual-notices-v1.json), [evidencia](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/METRICS.json) | Corpus/baselines/tiempos/fallos y revisión humana acotada; avisos4/4,FP0/1,FN0/3. Coste completo no reportado: desconocido, aceptado como límite de ejecución; no se inventa ni se deriva ganador. |
| 4.1 | Helper/guía→bot→usuario, origen y recuperación | [Código](../../apps/llm/src/delegation.rs), [evidencia](../../docs/verification/architect-communication.md) | Inbox/outbox y Presence median; contrato contextual corregido y4casos reales posteriores. |
| 4.2 | Instancias por rol con ajustes y permisos propios | [Código](../../apps/llm/src/architects.rs), [evidencia](../../.verification/contextual-notice-probe/819c9c43-9d8b-48f5-9975-4c940d8f5643/workspace.log) | Tres perfiles, límites/intersección, validación, rollback, migración/reapertura y personalización persistida. |
| 4.3 | Configuración básica/avanzada e integración aislada | [Código](../../frontend/lib/src/architects.dart), [evidencia](../../.verification/native-architect-save/d328ac65-ac4f-44d2-b26f-a73101292b53/CLOSURE.json) | Guardado/consulta de tres roles nativos; guard Unicode original fallido conservado y enmienda previa ASCII verificada. Pruebas Rust verifican reapertura/configuración. |

## Validación y versión

Fuentes vigentes cotejadas: 98 archivos Dart sin deriva y 216 entradas persistentes de la comprobación Rust/proveedor (incluyen corpus y planes), también sin deriva. Las 55 migraciones históricas comparadas mantienen sus bytes. [Auditoría estructurada](../../.verification/functional-closeout/e3788904-2ace-4020-8c21-5292facfe3d2/AUDIT.json).

Rust: **439 pruebas correctas y 29 optativas omitidas**, Clippy workspace/all-targets correcto. Flutter: **438 correctas y 49 optativas omitidas**, análisis correcto en su gate vigente; no hubo nuevos cambios Dart desde esa verificación. Formato, invariantes, firma y diferencias se registran en el cierre. Las pruebas omitidas no cuentan como ejecutadas: los recorridos instalados/modelos expresamente utilizados tienen sus resultados separados.

Último arreglo de producción: contrato de salida del bot contextual, con regresión roja/verde y cuatro casos posteriores con guía/bot reales. La [sonda real](contextual-notice-probe.md) acredita tres mensajes al origen y un registro sin mensaje, sin repeticiones al reabrir, sin efectos o cambios personales. Sus únicas fuentes nuevas posteriores al release son corpus y test optativo, excluidos del producto.

## Entrega y límites

[Paquete macOS arm64](../../.verification/functional-closeout/e3788904-2ace-4020-8c21-5292facfe3d2/Gennie-macos-arm64.zip), [SHA-256](../../.verification/functional-closeout/e3788904-2ace-4020-8c21-5292facfe3d2/SHA256SUMS) e [instrucciones](../../.verification/functional-closeout/e3788904-2ace-4020-8c21-5292facfe3d2/README.txt). El paquete se extrae en un temporal propio para comprobar firma y hashes de ejecutable, framework, backend y helper; luego se retira ese temporal. No se instala, inicia o despliega la entrega.

El coste completo de las inferencias no lo reporta el proveedor del bot y permanece desconocido; el coste parcial de guía se conserva sin inventar un total. La comparación empírica tiene diez casos de propuestas, una elección humana B y revisión asistida diferenciada; los avisos añaden cuatro acuerdos ficticios. No se declara superioridad general de formato o guía ni garantías universales del modelo.

No se migró la base personal, no se aplicaron propuestas personales, no se habilitaron bots adicionales o ejecución con Gennie cerrada. Los perfiles/runtimes/cuentas existentes siguen siendo requisitos para sus funciones; no se empaquetan secretos, bases o runtimes de modelos. No hay commit/push o despliegue.

Los [antecedentes del plan](../plans/gennie-chats-agentes-history.md) y registros de cada ensayo conservan sus fallos/versiones. Los límites históricos de accesibilidad no son trabajo pendiente. No se ha encontrado un requisito funcional pendiente dentro del alcance vigente.

El binario debug efímero del ensayo ya no está en target durante el último cotejo; no se ha aislado la causa. Su hash y ejecución quedaron registrados y se verificaron al cerrar la sonda. No falta una fuente ni el release entregado; no se reconstruye ni repite la campaña por esa ausencia.

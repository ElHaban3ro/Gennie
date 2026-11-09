# Comunicación de arquitectos: rutas y evidencia vigente

## Avances mediados por el bot

9 de noviembre de 2026. Los informes de avance libre del constructor (`Kind::Progress`) se entregan a la bandeja de delegaciones y ahora se redactan usando el perfil del bot y el contexto de la conversación de origen antes de llegar al usuario. El informe se trata como dato no confiable; no se copia como mensaje del constructor. Los estados operativos fijos y eventos legados conservan sus avisos fijos. Si falla la redacción, se informa la falta de detalle sin publicar el texto del constructor ni repetir la inferencia automáticamente.

Una regresión sobre el worker y SQLite comprobó primero que el avance se entregaba sin invocar el bot; tras el cambio, comprueba que el contexto y la observación llegan al mediador y solo su salida se entrega. Otras pruebas cubren redacción fallida sin filtrar el informe, mensajes idénticos suprimidos, avances distintos conservados, entrega transaccional y origen superado. Las inferencias adicionales por avances pueden aumentar consumo y latencia; la bandeja conserva su supersesión. La redacción sigue usando la sesión efímera del bot y los avisos se incorporan al hilo principal antes del siguiente envío explícito, como describe D-033: esto no acredita una conversación autónoma bidireccional entre el bot y el constructor ni una prueba nativa del flujo completo.

La suite `cargo test --workspace --locked`, el formato y las invariantes pasaron; el test de bootstrap que verifica el origen usa un aviso fijo para no iniciar inferencia en la suite automática. Clippy estricto encontró tres avisos en `apps/llm/src/lib.rs`, en cambios de otro ámbito ya presentes: dos `type_complexity` y un `collapsible_if`. El workspace pasó al exceptuar esas dos categorías; esa ejecución no equivale al Clippy estricto completo. No se inició una construcción ni una inferencia real para verificar este cambio.

29 de octubre de 2026, después de D-056. El requisito de origen exige helper/guía → bot → usuario para cualquier comunicación del arquitecto. Esta auditoría recorre los canales narrativos del producto actual y distingue sus datos de los artefactos técnicos que el usuario inspecciona. No usa un grep como prueba de ausencia ni declara el goal completo.

## Acceso a un trabajo bloqueado — 31 de octubre de 2026

El aviso más reciente de una construcción bloqueada ofrece abrir el sitio en Gennie Web y, después de abrirlo, volver a generar mediante la operación existente que conserva el trabajo anterior como origen. La acción es explícita y no presupone que un 403 sea un problema de sesión ni que el acceso lo resolverá. La referencia debe corresponder al chat, plugin, trabajo y último evento; un sucesor activo impide estas acciones. No se ha iniciado una construcción ni comprobado acceso real por esta corrección. La regresión Flutter cubre apertura y generación con backend ficticio, referencias antiguas y sucesor activo.

## Presentación en chat — 31 de octubre de 2026

Los avisos rutinarios de delegación se entregan con la voz del asistente, sin anteponer el nombre técnico del plugin. El chat retira ese prefijo al presentar avisos históricos con atribución de delegación, sin modificar el historial guardado ni las referencias que habilitan acciones. El resumen de herramientas identifica los fallos como intentos individuales: un intento posterior puede haber terminado correctamente. La revisión de una instancia real encontró dos llamadas fallidas y una completada al crear un encargo; el diario no conserva la causa libre de esos fallos. La creación del encargo no activa por sí sola una política de ejecución.

Regresión Flutter de aviso histórico y pruebas dirigidas de chat, evidencia y delegación; prueba Rust del mensaje rutinario. No se ha comprobado el cambio en la aplicación macOS en ejecución ni realizado inferencia nueva.

## Revalidación del alcance actual — 30 de octubre de 2026

Corrección posterior: la revisión de propiedad narrativa inferior no detectó una incompatibilidad del esquema de salida en el transporte contextual. La [regresión roja/verde](contextual-notice-contract.md) ahora recorre el envío stdio real con proveedor ficticio y las cuatro decisiones hasta el diario; corrige `action/reason/message` sin cambiar las rutas narrativas. Las campañas de borradores no prueban avisos entregados: [cobertura de métricas](assignment-metric-coverage.md). Se mantienen separados estos alcances y la calidad semántica aún no medida con guía/bot reales.

Se contrastaron los contratos de origen, worker del bot, proyección de trabajos y decisiones contextuales con los consumidores actuales. La reserva/fallo/cierre de redacción usa estados fijos del bot; la presentación de Plugins solo toma mensajes confirmados y ordenados del diario; la guía pasa su consejo al bot y la UI de actividad muestra el motivo validado del bot o un estado fijo. No se encontró una ruta narrativa nueva que publique observaciones del helper o consejo de guía como sustituto.

Los archivos críticos coinciden por hash con la puerta Rust vigente de 435 pruebas/28 optativas; los 211 archivos Rust/SQL/Cargo conservan conjunto y contenido salvo un test de opcionales de propuestas, idéntico al archivo de su prueba dirigida y Clippy correctos. Las 55 migraciones no cambiaron. La suite Flutter actual pasó 430/49; las modificaciones recientes afectan legibilidad, no estos contratos. [Cotejo y archivos inspeccionados](../../.verification/assignment-secondary-contrast/3b9fac67-8c62-4335-a60c-dc2af17270a4/ARCHITECT_AUDIT.json). No se ejecutaron modelos nuevos.

La nota de AGENTS se actualiza a rutas narrativas actuales auditadas. Esto cubre el flujo vigente y sus errores/recuperación comprobados; no garantiza toda redacción futura, fidelidad semántica universal o efectos externos. Los límites históricos que siguen conservan su fecha; campañas, migración y UI tienen informes posteriores propios.

## Rutas examinadas

| Superficie | Entrada y propietario | Texto que llega al usuario | Evidencia y límite |
|---|---|---|---|
| Chat: actualización del constructor | Plugins entrega Update al inbox de llm; worker de delegación prepara un mensaje mediante el bot o un estado fijo | Mensaje assistant confirmado por ConversationJournal/Presence, con trabajo/secuencia y origen | Pruebas de delegación: contexto acotado, origen retirado, rollback, no repetición, recuperación/reapertura. La redacción real conserva la muestra histórica de D-029; no se repitió inferencia aquí. |
| Ficha de construcción: resumen y actividad | Controller.present_job añade bot_presentation usando el diario de Presence | _JobPresentation consume solo mensajes confirmados o textos operativos fijos | Prueba presentation_is_read_only_rechecks_source_and_never_uses_helper_as_bot, fixtures UI y puente D-042; job.summary/events.message técnicos permanecen separados. Un origen inexistente no permite usar el informe como fallback. |
| Fallo al explicar un resultado del constructor | Error/reserva fallida/salida inválida/cierre del renderer del bot | Estado del trabajo y aviso de que falta la explicación; el enlace a Plugins se limita al estado | Fallo reproducido en tres regresiones existentes y corregido en una función común. Sin llamadas de modelo o uso del detalle privado para completar el aviso. No reescribe salidas listas/mensajes históricos. |
| Propuestas de Encargos | Guía devuelve probabilidades saneadas a Store; assignments_propose entrega los borradores y consejo al bot | Motivo/beneficio/preguntas son candidatos del bot; la UI solo indica recepción/fallo de guía | Store valida/sanea respuestas; assignment_proposals.dart no imprime guide. D-055 completó un turno/una guía/dos borradores en primer envío; no demuestra calidad, reparación o superioridad de formatos. |
| Avisos contextuales de Encargos | notices/guidance consulta guía consentida, entrega advice al renderer del bot y valida Decision | Mensaje del bot por el diario; actividad muestra acción/motivo del bot o estados fijos | guide_advice_goes_to_the_bot_and_only_bot_actions_reach_the_journal_once cubre record/notify/ask/offer_help, una consulta/redacción y recuperación. Falla/revocación no manda el consejo como sustituto. |
| Actividad: criterio de aviso | decision_details conserva consejo técnico separado de notice_reason/rendered_text | _NoticeDecision muestra reason solo sin failure_code; nunca serializa guide_advice | Fixture UI actualizado incluye una probabilidad distintiva: se ve el motivo del bot y no la cifra del consejo; consultar/actualizar no configura ni reconcilia. No demuestra comprensión del motivo por el modelo. |
| Bajo una burbuja: uso de herramientas | tool_use registra solo metadatos estructurales de llamadas reales | Nombre/función/estado y códigos fijos; sin argumentos, cuerpos o errores libres | Pruebas con PRIVATE_ARGUMENT/PRIVATE_RESULT/PRIVATE_ERROR y los recorridos FRB D-043. No se usa esta vía para narrar informes del helper o consejos de la guía. |
| Mensajes antiguos de plugins | Historial ya confirmado, con validación de referencia en chat_message.dart | Texto histórico conservado; los campos técnicos se comparan para resolver una acción, sin convertirlos en mensajes nuevos | Relectura de los dos consumidores de summary/events. No se borran ni reescriben mensajes legados; no confundir preservación de historia con una vía nueva de comunicación del helper. |

## Artefactos y campos técnicos que permanecen visibles

El catálogo presenta títulos/descripciones, etiquetas/contratos de parámetros y código de funciones publicados. La ficha de una ejecución permite expandir sus parámetros y resultado técnico. Son datos de la función y del intento, con sus permisos/revisión; el usuario necesita inspeccionarlos para autorizar/usar el plugin. La UI no debe atribuirlos a una explicación confirmada del bot ni usarlos como petición de intervención del helper. Se conservaron estas rutas durante la revisión; no se añadió un permiso de exponer informes o preguntas libres por incluirlos en un artefacto.

La salida técnica de ejecución procede del ejecutor web/MCP, no de la valoración de la guía: la notificación contextual se adjunta por separado. guide_advice permanece en la proyección técnica para auditoría, pero las vistas examinadas no lo imprimen. La existencia de ese campo en FRB/SQLite no equivale a una nueva comunicación al usuario; tampoco constituye una barrera de confidencialidad frente al propietario de su base local.

## Cierre local y límites

84 pruebas LLM correctas, diez optativas omitidas; workspace Rust 406 correctas/27 omitidas, Clippy workspace/all-targets, formato e invariantes correctos. Diez pruebas UI de avisos contextuales correctas, incluida la exclusión del consejo distintivo, sin inferencia o efectos. El arreglo de texto no cambió Dart del producto, API, migraciones, dependencias, estilo general o barra lateral. La suite Flutter completa conserva las 362 pruebas/49 optativas de D-056; no se repite por una cadena Rust y un fixture de prueba.

Biblioteca/helper debug y paquete release compilados secuencialmente por los lanzadores; firma ad hoc verificada. La instancia Debug del usuario permaneció abierta; no se instaló ni inició la aplicación nueva. Logs delegation-fallback-* y delegation-fallback-finish.json en .verification/assistant-chat-real. No hay nueva campaña autorizada: la muestra D-055 cerró tras consumir su único turno/guía. Otro recorrido semántico necesita fuentes/binarios actuales y permiso nuevo; no se reutilizan muestras anteriores como si fueran equivalentes después del cambio Rust.

HUMAN_REVIEW.md bajo el run D-055 muestra las dos variantes de ese caso ficticio con todos sus campos modificados y la rúbrica, ocultando etiquetas de origen. human-review-key.json conserva la correspondencia; el contenido puede permitir inferirla y no se afirma doble ciego. Solo se solicita un juicio humano concreto; no se completan puntuaciones desconocidas o aceptación implícita, ni se aplica un borrador por elegir una variante. ratings.csv sigue sin puntuaciones humanas.

La auditoría respalda las rutas narrativas inspeccionadas, sus fallos y recuperación. No acredita la fidelidad semántica de toda redacción futura, revisión humana del catálogo, resultados físicos de ejecutores, comprensión real de citas/Detener, comparación empírica de formatos o matriz nativa/VoiceOver. Esos requisitos del plan permanecen abiertos con su evidencia propia; no se declaran cumplidos por este cierre.

## Causas de llamadas — 31 de octubre de 2026

Presence 0020 añade almacenamiento de causas para mensajes fijos reconocidos de validación de Encargos; se presentan en el detalle del uso. Los fallos externos se clasifican mediante estados estructurados o patrones acotados en memoria y se conserva únicamente una descripción fija; si no se reconoce la causa se guarda esa limitación. No se persiste texto libre externo. No se recuperan causas descartadas antes de esta modificación. Pruebas dirigidas de extracción, lectura SQLite y presentación Flutter; no se ejecutó una sesión real con el cambio.

La clasificación de texto libre es orientativa y no garantiza identificar todos los errores; no se inspeccionan argumentos ni respuestas exitosas. Pruebas con errores MCP directos y mediante proxy, tokens, URL y correos ficticios verifican que solo se conserve la categoría.

Cuando un intento MCP devuelve error y requiere revisión se conservan tanto el código de revisión como la causa clasificada; ninguno reemplaza al otro.

## Error MCP externo y detalle Flutter — 31 de octubre de 2026

Se consultó el servidor público MCP DeepWiki por HTTPS, sin cuenta, y se envió una lectura con un identificador ficticio mal formado que el servidor devolvió dentro del error. La respuesta real (isError=true) se pasó al clasificador de producción: la regresión observó «formato de un dato no es válido» y comprobó que el centinela ficticio no aparecía en el diagnóstico. Se amplió la clasificación para este mensaje, que inicialmente quedaba sin causa.

La cadena clasificada se pasó por un archivo temporal a una sonda separada: el diario Rust la guardó en SQLite temporal y la UI Flutter con FRB real mostró el detalle en cuatro combinaciones claro/oscuro y tamaño normal/reducido. El volcado lógico de esa base no contenía el centinela. Se migró Presence a 0020; integridad referencial y seis pruebas de la sonda pasaron. La base temporal se retiró.

La petición HTTP se hizo fuera del proceso Gennie y la notificación del proveedor se reconstruyó en una prueba Rust; el resultado clasificado se inyectó explícitamente al diario de la sonda. Esto no acredita un recorrido continuo desde el chat con un proveedor Codex real, ni un servidor conectado y llamado desde la instancia abierta, ni una inspección nativa del runner macOS. La comprobación usa un centinela ficticio y no demuestra protección universal para cualquier respuesta. La instancia abierta y su base no se modificaron.

## Primera prueba desde el chat con MCP real

Se ejecutó una instancia macOS de QA con base aislada y DeepWiki configurado con permiso exclusivo de lectura de estructura. Desde el chat, el bot consultó el catálogo y realizó una llamada a read_wiki_structure con un identificador ficticio mal formado. El servidor devolvió error de formato y el host marcó el intento incierto. La primera ejecución reveló que el diario guardaba result_uncertain pero no la causa; se corrigió el transporte para conservar ambas en detail y failure_reason, y para leer el bloque MCP nativo de error en vez de los metadatos; las regresiones Rust y Flutter reproducen ambos problemas. El identificador se introdujo deliberadamente en el mensaje del usuario; la comprobación de sensibilidad corresponde al registro de herramienta, no al texto literal del chat. El usuario revisó el primer intento como no aplicado y autorizó continuar la prueba.

### Repetición completa con la corrección

Tras la revisión humana, se cerró limpiamente la primera instancia QA y se abrió otra compilación macOS release con la misma base temporal. El chat consultó el catálogo de DeepWiki, verificó la revisión y ejecutó una sola lectura con un identificador ficticio inválido. El servidor devolvió el error real. Presence guardó el estado failed, detail=result_uncertain y failure_reason con la causa fija de formato inválido. El detalle real del chat mostró tanto esa causa como el aviso de revisión.

La consulta de las tablas presence_tool_uses, plugin_mcp_attempts y plugin_mcp_sources no encontró el centinela ficticio; este sí estaba en tres mensajes escritos explícitamente por la prueba, no en respuestas del asistente. SQLite informó integrity_check=ok y ninguna violación de claves. La nueva llamada quedó incierta y no se repitió ni se resolvió automáticamente. Se comprobó salida limpia de la instancia QA; la aplicación y la base habituales permanecieron intactas. La evidencia cubre esta respuesta y este centinela, no demuestra identificación universal de errores ni ausencia de cualquier dato sensible en el historial conversacional.

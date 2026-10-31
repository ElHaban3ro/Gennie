# Índice de decisiones

Estado actual: escritorio Flutter + flutter_rust_bridge; ver [arquitectura](../architecture/overview.md) y [UI](../../UI.md). Los documentos anteriores conservan decisiones históricas; no reintroducir la presencia flotante ni sus sondas.

- [Arquitectos D-029](architects.md): configuración por rol y mediación del constructor al chat mediante el bot; auditoría completa pendiente.
- [Propuestas de Encargos D-030](assignment-proposals.md): selección humana, borradores del bot, guía opcional y aplicación transaccional; evaluación semántica pendiente; ejecución/resultados continúan en D-031/D-032.

- [Plugins web D-027](web-plugins.md): recetas HTTP/DOM verificadas, sesión bajo demanda y plan del constructor aislado.
- [Plugins MCP D-028](mcp-plugins.md): conexiones, catálogo/permisos, gestión visual y diario de revisión de llamadas sin inferencia; comprobación nativa pendiente.

- [Navegador persistente D-026](browser.md): perfil exclusivo, apertura desde chat, herramientas compatibles con Playwright mediante Patchright y preparación del runtime local.

- [Encargos y MCP modular D-025](assignments.md): registro de planes y políticas contextuales de seguimiento, preparación/aviso separados y transporte local compartido.

- [Historial propio D-024](chat-history.md): catálogo local, mensajes por conversación y consulta desde la sidebar.

- [Composición de la cara sobre el escritorio D-023](wallpaper-face.md): material nativo global, ajustes de apariencia y límites de las capturas.

- [Reminders plugin](reminders-plugin.md): integración local EventKit y encargos de cambios.

- [Voz de salida de Gennie D-022](tts.md): Kokoro MLX local, preferencias y reproducción de respuestas.
- [Almacén compartido de modelos](model-storage.md): rutas globales por usuario y migración conservadora.

- [Conversación de Gennie D-021](llm-conversation.md): destino predeterminado de la voz, hilo único y estado de conexión.

- [Operación musical D-020](music-routing.md): separación de operación y aplicación, música genérica, veto de efectos y límites comprobados.

- [Ajustes de Laya D-019](laya-settings.md): precarga, plazo configurable y conservación en memoria, independientes de Nemotron.

- [Auditoría de experiencia de voz](../verification/voice-experience.md): correcciones de estado/inventario, 220 casos con modelo real y límites pendientes.

- [Precarga y apps instaladas D-018](installed-apps.md): readiness de Laya e inventario dinámico de destinos.

- [Control semántico D-017](semantic-control.md): reemplaza la gramática de frases, separa disponibilidad y prepara adaptación/revisión de voz.

- [Control por voz D-016](control.md): Laya, autorización, ejecutores comprobados y límites del lenguaje libre.

- [Dictado local D-015](transcription.md): atajo, globo temporal, descarga y ciclo de vida de Nemotron.

- [Spaces D-014](spaces.md): pertenencia de la presencia a escritorios macOS.
- [Cara monocromática D-013](monochrome-face.md): corrección estética explícita del usuario.
- [Base conjunta D-012](base.md): arquitectura inicial y adaptaciones históricas.
- [Versiones, licencia y avisos](dependencies.md): comprobación 2026-10-11 y límites.
- [Historial D-001..D-011](initialization-history.md): respuestas/decisiones originales; D-009 reemplaza ubicación D-001 y D-010 la demo aislada.

Requisitos confirmados tienen prioridad sobre supuestos gráficos delegados. Las propuestas históricas no se convierten en autorizaciones de publicación ni acciones externas.

- [Proactividad bajo demanda](proactivity.md): cliente de decisiones de OpenRouter sin rutinas automáticas.

- [Ejecución local de Encargos D-031](assignment-execution.md): calendario, activación humana, diario y recuperación sin repetir efectos inciertos.

- [Resultados de Encargos por el bot D-032](assignment-notices.md): consentimiento, outbox/inbox, referencias e interfaz de revisión compartida.

- [Contexto de avisos en el proveedor D-033](provider-context.md): reserva por sesión, incorporación nativa antes del siguiente envío y recuperación conservadora.

- [Valoración contextual de avisos D-034](contextual-notices.md): consentimiento por preparación, guía→bot, registro sin mensaje y retirada local.
- [Refinamiento conversacional D-035](proposal-refinements.md): respuesta real, versiones de borrador y aplicación humana explícita.
- [Historial de revisiones D-036](proposal-history.md): archivo humano por familia y consulta por páginas sin borrar borradores/recibos.
- [Peticiones del chat D-037](chat-review-requests.md): borradores locales con fuente real, sin formulario previo ni consentimiento de guía inferido.
- [Evaluación de Encargos D-038](assignment-evaluation.md): corpus reproducible, formatos sin pérdida y pilotos ejecutados sin atribuir calidad a invariantes; comparación semántica amplia y revisión humana pendientes.
- [MCP manual D-039](manual-mcp.md): permisos persistidos sin conversación activa y recorrido Flutter/FRB/stdio con incertidumbre/revisión sin reenvío.
- [Copia histórica D-040](historical-migration.md): migración/recuperación en copia real, preservación lógica de datos y medida de paginación.
- [Inventario MCP D-041](plugin-inventory.md): proxies del bot sin exigir navegador/Encargos; hallazgo de mediación narrativa continuado en D-042.

- [Ficha del constructor D-042](builder-messages.md): mensajes confirmados del bot por origen/trabajo/secuencia; estado del host sin narración libre del helper.

- [Contenido MCP nativo D-043](native-mcp-content.md): bloques multimodales separados del JSON local, error conservado y reducción sin pérdida de texto remoto.

- [Diario web D-044](web-attempts.md): reserva antes del efecto, incertidumbre entre candidatos/reinicios y revisión humana sin reenvío.

- [Cancelación de transportes D-045](transport-cancellation.md): EOF y cierre por propietario, clientes aislados y diarios de incertidumbre; control Detener del chat continuado en D-047.

- [Contrato de borradores D-046](proposal-contract.md): esquema alineado con rechazos del piloto, cadencia/preguntas explícitas y backend independiente, sin nueva campaña.

- [Detener respuesta D-047](chat-cancellation.md): identidad por entrada, cancelación explícita y fragmento sellado con estado en Presence 0019; navegación y estilo generales conservados.

- [Accesibilidad nativa D-048](native-accessibility.md): nombres en botones/campos, atajo multilínea explícito, capturas equivalentes y prueba de Quit; límites de modificadores sintéticos y VoiceOver documentados.

- [Cierre nativo D-049](native-termination.md): retiro del delegado tras aprobación del backend, rechazos recuperables y comprobación de canales sin avisos posteriores al apagado.

- [Portabilidad de propuestas D-050](proposal-portability.md): campaña ampliada inconclusa, permiso consumido y alternativas completas sin debilitar validación.

- [Teclado del perfil D-051](profile-keyboard.md): Escape protegido durante IO, foco restituido y comparación visual macOS con ratón.

- [Contrato descubierto D-052](evaluation-contract-gate.md): comparación exacta de esquemas antes de generar y prueba breve con detención ante fallo, sin permiso live implícito.

- [Foco de citas D-053](quote-navigation-focus.md): original localizable y operable por teclado, nodos liberados y comprobación nativa entre sesiones sin cambiar la activa.

- [Presupuesto de sesiones D-054](chat-session-budget.md): máximo tres sesiones distintas por página, avisos intercalados y copia histórica actualizada hasta Presence 0019.

- [Errores de propuestas D-055](proposal-validation-feedback.md): índice/campo antes de guardar, corrección explícita sin repetir guía y comprobación de un turno separada de la comparación.

- [Teclado de revisiones D-056](review-keyboard.md): Escape protegido durante guardado, foco de vuelta a propuestas y controlador de scroll explícito en macOS.

- [Arquitectos ampliables D-057](expandable-architects.md): pestaña propia, tres bases preservadas y catálogo reutilizable de múltiples instancias/tipos.

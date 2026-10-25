# Índice de decisiones

Estado actual: escritorio Flutter + flutter_rust_bridge; ver [arquitectura](../architecture/overview.md) y [UI](../../UI.md). Los documentos anteriores conservan decisiones históricas; no reintroducir la presencia flotante ni sus sondas.

- [Historial propio D-024](chat-history.md): catálogo local, mensajes por conversación y consulta desde la sidebar.

- [Composición de la cara sobre el escritorio D-023](wallpaper-face.md): material nativo global, ajustes de apariencia y límites de las capturas.

- [Reminders plugin](reminders-plugin.md): integración local EventKit y eventos de cambios.

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

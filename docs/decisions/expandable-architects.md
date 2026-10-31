# D-057 — Arquitectos ampliables y navegación propia

2026-10-31. Petición del usuario: Arquitectos tiene su propia pestaña conservando el estilo actual. Mantener los arquitectos base y permitir tantas instancias y tipos como se necesiten, reutilizables desde otros lugares.

## Catálogo e identidad

Se conservan `gennie`, `plugin_builder` y `proactivity_guide` con sus consumidores, tipo y proveedor actuales. Sus ajustes siguen editables y sus identidades no se pueden eliminar. Nombre de configuración separado del nombre/avatar del chat en Presence. Una instancia adicional de tipo bot no sustituye a Gennie ni crea automáticamente un contacto.

Cada instancia adicional tiene ID propio generado por el host, nombre, tipo libre, revisión y configuración de proveedor/modelo, personalidad, instrucciones y permisos. El tipo no es único: se pueden guardar varios bot, helper, guide o tipos personalizados. El proveedor conserva sus contratos reales: Codex usa modelo/esfuerzo y permisos; Decisions mantiene sus modelos integrados, sin herramientas o esfuerzo. Etiquetar una instancia no crea un motor, herramientas o canal de comunicación nuevo.

`load_by_id` y `runtime_settings_by_id` permiten que otros flujos referencien explícitamente un arquitecto. Los ajustes efectivos Codex se limitan por los permisos de Gennie y rechazan perfiles deshabilitados o de otro proveedor. Esta entrega prepara el catálogo reutilizable; los flujos existentes siguen conectados a sus bases. No añade ejecutores, rutas automáticas, chats o decisiones remotas.

## Persistencia y API

La migración 0010 añade nombres a las bases y `llm_architect_instances` para las adicionales, con una vista conjunta `llm_architect_profiles`. Mantener las bases en su tabla conserva las FK de delegaciones y avisos sin reconstruir o borrar esos datos. 0001..0009 no se modifican.

La API JSON humana existente admite list/get/create/save/remove. Crear genera identidad/revisión en el host; guardar confirma revisión esperada y eliminación solo admite instancias adicionales con revisión vigente. Solo editar `gennie` actualiza `llm_preferences` y el worker activo; otro bot no modifica ese espejo. Las mutaciones base conservan el worker y gate del constructor. El catálogo adicional se puede gestionar con conversación apagada, desde el coordinador, sin iniciar modelos. No se cambia la firma FRB ni se editan fuentes generadas.

## Interfaz y verificación

Arquitectos se añade después de Plugins con el mismo RailButton/SVG/medidas/estados. Su lista reutiliza AppSidebar, búsqueda y SidebarTile; el editor usa AppDetailView. Nuevo prepara un borrador sin permisos ni habilitación; Duplicar copia la configuración en una identidad nueva deshabilitada. Se confirma con Crear arquitecto. Bases marcadas, tipos editables solo en adicionales, opciones avanzadas plegadas, borradores por instancia y errores conservados. Guardar bloquea navegación y doble envío. Configuración mantiene sus ajustes generales y deja de contener el panel.

Pruebas locales cubren migración con referencias existentes, bases conservadas, instancias repetidas/personalizadas, permisos, revisiones, API sin LLM y reapertura sin hilos/mensajes/trabajos. Widgets y capturas software comprueban navegación y formulario a tamaño normal/mínimo en ambos temas. No acreditan integración nativa ni ejecución de consumidores futuros. Sin campañas de modelos, pruebas de accesibilidad, paquete macOS, instalación o cambios personales.

Resultado: **204 pruebas Rust correctas, 22 optativas omitidas**, en LLM/Plugins/bootstrap; **15 widgets correctos** (11 del catálogo y 4 de navegación/ajustes). Análisis Flutter, Clippy de los tres paquetes/todos los targets, formato e invariantes correctos. Se inspeccionaron ocho capturas software del formulario base/nuevo en ambos temas y tamaños. [Captura de la pestaña](../../.verification/architects-expansion/20261031/architects-tab-dark-1100.png), [logs Rust](../../.verification/architects-expansion/20261031/rust-final.log) y [widgets](../../.verification/architects-expansion/20261031/flutter-tests.log). La aplicación personal no se inició, actualizó ni migró.

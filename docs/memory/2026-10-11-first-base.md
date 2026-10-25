# Primera base — 2026-10-11

> Contexto histórico: la UI descrita aquí fue retirada al migrar a Flutter. La cara, presencia flotante, globos y sus sondas no forman parte del producto vigente. Para arquitectura y verificación actuales, consultar los documentos de arquitectura y reglas del índice. Las migraciones y los datos conservan su historia.

Estado final: base construida en checkout main, sin commits/push ni cambios de remoto. Prompt original SHA-256 7412501ba941c357418096079a62cff4487dccf9cb138f197c36f4088ea3d9dd, leído íntegro junto con respuestas/decisiones/plan antes de código. No había código ni AGENTS de ancestros aplicables. Historial consolidado conservando pendientes antiguos resueltos. Corrección visual posterior D-013 aplicada: monocromática, moderna y minimalista.

Verificación final: fmt/check, 13 tests, Clippy -D warnings, build, script invariantes/enlaces. Se comprobaron SQLite/migraciones, registro/DAG, configuración/precedencia y error/confirmación de worker. Resultado y comandos precisos en [implementation](../initialization/implementation.md).

Runtime con base aislada .verification/review-final.sqlite3, Cara dormida y feliz, zetas con cambios y expansión en capturas sucesivas, M persistido (lectura SQLite=1), reapertura misma DB sin migraciones repetidas, A/AboutSlint accesible, Escape y clic secundario finalizaron con logs module_stopped/shutdown_complete. Últimos PIDs propios de app/sonda comprobados ausentes. Primera ubicación no funcionaba porque Winit crea handle al entrar event loop; corregido esperando window.winit_window().await y log ui_ready solo después de posicionar.

Herramientas: Orca status not_running/runtime_unavailable; usado cua_repl nativo disponible. Capturas iniciales de sonda regular fallaron SCStreamError -3811; sonda Accessory/sin marco accesible. Clicks dirigidos a app específica no son prueba válida de prioridad/paso OS; no se afirma éxito de click-through ni composición alfa a partir de esa sonda. Falta revisión manual de foco, always-on-top, transparencia sobre contenido real, interacción fuera y reentrada rápida; no multi-monitor/Retina/Spaces/otros OS. No review continuo de animación.

Dependencias verificadas en crates.io/mantenedor y RustSec. bincode transitivo sin mantenimiento vía typed-index-collections presente macOS; xml-rs/ttf-parser avisos en lock completo pero no grafo efectivo macOS. No cargo audit instalado ni auditoría supply-chain. Revisar al actualizar. Ninguna integración ChatGPT/Grok/voz/correo implementada; viabilidad y autonomía futuras pendientes, no bloqueo de esta base.

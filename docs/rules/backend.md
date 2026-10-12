# Servicios Rust y recursos

Alcance: kernel/core y servicios de apps; composición en bootstrap. Leer [modules](modules.md) al cambiar contratos o lifecycle y [database](database.md) al tocar persistencia.

## Responsabilidades
La app posee sus modelos, repositorio y presentación. `repository.rs` concentra SQL de preferencias; `lib.rs` coordina el módulo y callbacks; `desktop.rs` adapta la política de ventana. Separa por responsabilidad cuando crezca; no introduzcas una capa por cada operación pequeña. No hay handlers HTTP, paginación, autenticación ni routers. Si aparecen requisitos de red, diseñar y documentar contratos antes de implementarlos; no trasladar automáticamente patrones de servidor al escritorio.

AppState se construye una vez en startup y contiene handles clonables de configuración, DB y Shutdown. No añadir estados de cara, credenciales o repositorios de negocio. Recursos específicos por constructor o en la propia app. Config usa Arc; DB clonada comparte conexión. Kernel es independiente de Slint.

## Errores y tareas
Propaga errores con contexto sin valores sensibles. Logging actual usa campos `event`, `module`, `version`; no publicar URLs, tokens ni contenido de usuario. Un fallo de DB o UI impide declarar arranque correcto. No ocultar fallos persistentes con defaults.

El event loop y objetos Slint viven en main. SQL de escritura interactiva corre en un worker con cola acotada, sin IO de red. No bloquear animaciones con trabajos extensos. La cola debe confirmar commit antes de presentar éxito; errores visibles y logs. Stop desconecta productores, drena y une el worker antes de cerrar DB. No lanzar tareas detached. Para futuros workers largos definir cancelación propia y tiempo máximo; no prometer cierre acotado sin implementarlo.

Fechas SQL: UTC `strftime('%Y-%m-%dT%H:%M:%fZ','now')`. Nombres técnicos en inglés; Optional explícito. No eventos redundantes para interacciones que ya actualizan su propia vista.

## Verificación
Cambios compartidos requieren tests de consumidores y cierre tras errores. No inventar pruebas HTTP en esta app. Ver [testing](testing.md).

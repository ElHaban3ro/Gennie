# Módulos y dependencias

Propietario del grafo, workspace, registro, AppModule y lifecycle. Workspace edición 2024, resolver 3, versiones centralizadas exactas; crates heredan package/dependencies. Kernel no importa apps/contracts. Apps pueden importar kernel y otras apps solo sin ciclos. Contracts solo cuando rompa un ciclo real; no generar capas/crates vacíos.

## Registro y orden
Registro explícito en bootstrap/registry.rs; no discovery de módulos por DB, red ni filesystem. AppModule Send+Sync expone nombre estable, activación inicial, dependencias obligatorias/opcionales, migraciones y start/stop. No incluir HTTP, permisos o pestañas sin consumidor.

Nombres: minúsculas ASCII/dígitos/underscore; rechazar duplicados. No normalizar guiones, así no hay colisiones ambiguas. Validar DAG antes de migrar apps. Obligatorias ausentes son errores de código; apagadas son errores de configuración. Opcionales ausentes son válidas; presentes ordenan aunque estén apagadas. Fila huérfana no representa código disponible. Insertar módulo no sobrescribe selección persistida. Migrar todos los compilados, iniciar solo activos. No hay activación en caliente en este hito: modificar selección requiere reiniciar. Implementar hot activation solo con transición serializada, gate y compensaciones definidos si se solicita.

## Lifecycle y comunicación
Start no entra en el event loop, es idempotente y serializado; stop detiene solo recursos propios y es idempotente. UI inicia explícitamente en main después de start. Si falla start, limpiar ese módulo y los iniciados en orden inverso, conservar error original y registrar secundarios. Cierre de UI solicita shutdown, detiene módulos y cierra DB al final.

Presencia consume Controllers públicos de transcription y llm mediante dependencias opcionales acíclicas. Decisions permanece registrado, pero no se conecta a Presence ni recibe transcripciones. `gennie-llm` posee la conversación de Gennie y aísla el proveedor Codex en `codex.rs`; bootstrap solo compone y entrega el Controller a presencia. Para nuevas comunicaciones: llamada pública si dependencia real acíclica; trait/DTO en contracts para ciclo; bus para hechos sin respuesta. Un broadcast es efímero y acotado: manejar lag y reconstruir desde fuente, publicar después de commit, no prometer entrega durable. No convertir tablas de otras apps en interfaz implícita.

`gennie-tts` es una dependencia opcional de presence, compuesta por bootstrap y sin ciclo. Posee su Controller y recursos propios; ver [D-022](../decisions/tts.md).

`gennie-reminders-plugin` es una app independiente registrada en bootstrap, sin dependencias de otras apps ni tablas. Su Controller bloqueante debe consumirse fuera de main. El observador EventKit pertenece al módulo y se detiene antes del cierre; ver [diseño](../decisions/reminders-plugin.md).

## Dependencias
Antes de añadir/actualizar: verificar versión exacta en registro oficial y mantenedor, MSRV/plataforma/features/licencia y avisos RustSec o del mantenedor. Guardar fuentes, fecha y límites en docs/decisions/dependencies.md. Lockfile versionado; no afirmar ausencia de vulnerabilidades porque no haya avisos. Revisar tránsito y artefacto objetivo; no habilitar Qt, servidores, red o tray sin necesidad. Winit unstable API se aísla en desktop.rs y requiere reevaluar al actualizar Slint.

## Añadir una app
1. Registrar requisito y responsabilidad, tablas propias, dependencias y recursos en plan/decisión vigente.
2. Leer architecture/overview y modules, backend y testing; database/UI/configuration según alcance.
3. Crear crate significativo con interfaz pequeña, no andamiaje vacío; añadir dependencias centrales y registro.
4. Validar ciclos, activación, migraciones incluso apagada, lifecycle y limpieza tras fallo.
5. Actualizar mapa/docs en el mismo cambio. Composición solo en bootstrap; SQL y secretos según documentos propietarios.

El backend directo i-slint-backend-winit tiene exactamente la versión de Slint; su configuración y adaptación permanecen en desktop.rs. El menú nativo compartido pertenece a status_menu, sin introducir Tauri ni otro event loop.

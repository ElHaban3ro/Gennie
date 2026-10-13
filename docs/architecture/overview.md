# Arquitectura vigente

D-018 añade precarga asíncrona de Laya e inventario de apps instaladas como destinos de open_app, dentro de decisions. No hay nuevos componentes de kernel, migraciones ni servidores. [Diseño](../decisions/installed-apps.md).

D-019 añade preferencias propias de Laya en SQLite y ajustes separados en la presencia. El worker existente procesa su guardado; no se añade infraestructura de kernel. [Diseño](../decisions/laya-settings.md).

soc es un monolito modular Rust de escritorio. Primer hito local macOS; arquitectura orientada a más plataformas, sin prometer soporte probado. El producto futuro es un asistente personal proactivo: esta entrega incorpora presencia, dictado local y control acotado de aplicaciones/música mediante Laya; aún no conecta cuentas.

```
bootstrap → soc-presence, soc-transcription, soc-decisions
soc-presence → soc-transcription, soc-decisions, soc-core, soc-db
soc-transcription → soc-core, soc-db
soc-decisions → soc-core, soc-db
soc-core → soc-config, soc-db
```

Kernel es infraestructura independiente del producto. Bootstrap configura una única conexión, construye registro explícito, valida DAG, registra selección, migra todos los compilados y empieza activos. La UI corre en main; start abre worker de persistencia y retorna. El worker usa la misma DB, SQL fuera del event loop y confirma éxito después del commit. Shutdown persistente, stop inverso y close compartido al final. La presencia base no requiere red. Transcripción usa un proceso hijo propio para captura/inferencia y otro para descargar el modelo por HTTPS; no hay listener ni puertos.

SQLite usa una conexión serializada en lugar de pool. Módulos tienen historia/checksums propios; SQL de apps vive con ellas. Selección de activación se conserva; apagar presence en DB se observa al reiniciar, termina sin UI. No UI de administración ni hot activation. Reduced motion es preferencia de presence; estado visual solo memoria. No contracts porque no hay ciclos de consulta; no buses porque no hay comunicación externa que requiera ellos.

Winit se aísla en apps/presence/src/desktop.rs: política macOS Accessory, transparencia, nivel, posición y pertenencia de la presencia a Spaces. El icono y menú de barra superior macOS pertenecen a apps/presence/src/status_menu.rs; sus acciones usan los callbacks de la UI y el cierre normal. La UI usa Slint declarativo, interpolación suave de tamaño/gestos y movimiento ambiental con ritmos independientes; respeta movimiento reducido. Detalles visuales se registran como supuestos revisables. Integraciones OS futuras deben ser interfaces justificadas por capacidades, no crates vacíos.

Contratos y propietarios detallados: [modules](modules.md). Requisitos: [initialization](../initialization/requirements.md). Evidencia: [memory](../memory/index.md). Limitaciones de seguridad de dependencias: [dependencies](../decisions/dependencies.md).

La integración D-015 añade `bootstrap → presence → transcription → core/db` sin ciclos. El modelo no forma parte de kernel; procesos hijos cancelables contienen red o recursos ASR. Globo temporal Slint junto a la cara conserva su posición/hit test y política de Spaces. [Diseño y límites](../decisions/transcription.md).

D-016 añade decisions como app independiente: helper local Laya por IPC sin puertos, autorización de órdenes y ejecutores OS con comprobación. La voz usa el atajo actual, habilitado para órdenes al abrir; dictado sigue disponible desmarcando el control en ajustes. Kernel no conoce modelos, catálogo ni diario. [Diseño y límites](../decisions/control.md).

D-017 sustituye la gramática por interpretación semántica, separa intención/disponibilidad y añade adaptación local con particiones por grupos y revisión de voz sin efectos. Los candidatos entrenados no demostraron cobertura fiable suficiente y no se activaron. [Diseño vigente](../decisions/semantic-control.md) y [mediciones](../verification/semantic-control.md).

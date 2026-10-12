# Arquitectura vigente

soc es un monolito modular Rust de escritorio. Primer hito local macOS; arquitectura orientada a más plataformas, sin prometer soporte probado. El producto futuro es un asistente personal proactivo: esta entrega aún no toma decisiones, ejecuta acciones OS ni conecta cuentas.

```
bootstrap → soc-presence → soc-core → soc-config
                       ↘          → soc-db
                         soc-db
```

Kernel es infraestructura independiente del producto. Bootstrap configura una única conexión, construye registro explícito, valida DAG, registra selección, migra todos los compilados y empieza activos. La UI corre en main; start abre worker de persistencia y retorna. El worker usa la misma DB, SQL fuera del event loop y confirma éxito después del commit. Shutdown persistente, stop inverso y close compartido al final. No hay procesos externos, listener ni red.

SQLite usa una conexión serializada en lugar de pool. Módulos tienen historia/checksums propios; SQL de apps vive con ellas. Selección de activación se conserva; apagar presence en DB se observa al reiniciar, termina sin UI. No UI de administración ni hot activation. Reduced motion es preferencia de presence; estado visual solo memoria. No contracts porque no hay ciclos de consulta; no buses porque no hay comunicación externa que requiera ellos.

Winit se aísla en apps/presence/src/desktop.rs: política macOS Accessory, transparencia, nivel, posición y pertenencia de la presencia a Spaces. La UI usa Slint declarativo y animación spring de la versión fijada. Detalles visuales se registran como supuestos revisables. Integraciones OS futuras deben ser interfaces justificadas por capacidades, no crates vacíos.

Contratos y propietarios detallados: [modules](modules.md). Requisitos: [initialization](../initialization/requirements.md). Evidencia: [memory](../memory/index.md). Limitaciones de seguridad de dependencias: [dependencies](../decisions/dependencies.md).

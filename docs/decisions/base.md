# D-012 — Base conjunta nativa

- Fecha: 2026-10-11. Estado: aceptada como elección técnica delegada; detalles visuales provisionales revisables.
- Origen: alcance autorizado del usuario y diseño del asistente. Reemplaza pendientes históricos de enrutamiento y toolkit; mantiene D-003..D-011.
- Decisión: cinco crates útiles, Slint/Winit/FemtoVG, configuración .env explícita, SQLite compartido, migraciones por módulo con checksum, lifecycle/registro central y presence dueña de UI.
- Motivo: base real junto a cara; sin servidor ni recursos vacíos. Una conexión compartida serializada en lugar de pool PostgreSQL, sin buses/contratos/ConfigSource transversales sin consumidor. SQL rusqlite en lugar de sqlx porque esta carga es local mínima; migraciones embebidas con bloqueo transaccional e inmutabilidad verificadas.
- Alternativas: demo aislada rechazada por corrección del usuario; WebView excluido; infraestructura HTTP del ejemplo inaplicable; motor físico innecesario para spring.
- Datos: modules activa/desactiva al reiniciar; reduced_motion único ajuste propio. No se persiste estado activo.
- Portabilidad: adaptación OS aislada en desktop.rs; solo evidencia macOS en este hito.
- Supuestos: 80/168 px, grafito/blanco/gris sin mejillas (D-013), esquina inferior derecha, Dock reservado; evaluación del usuario puede cambiarlos.
- Revisión: feedback visual, nuevos módulos/concurrencia, otras plataformas o distribución. No se promete activación caliente ni rollback binario.

Fuentes de versiones/licencia/avisos en [dependencies](dependencies.md). Origen en [historial](initialization-history.md).

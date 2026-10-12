# soc

Primera base local del asistente personal: presencia nativa flotante con cara siempre visible, kernel y SQLite. Rust + Slint, sin WebView. Primer hito macOS; otros sistemas aún no verificados. No conecta ChatGPT/Grok ni realiza tareas OS. La presencia está configurada para acompañarte entre escritorios macOS; al actualizar debes cerrar y volver a abrir tu instancia para activar cambios de ventana. Pantalla completa tiene elegibilidad auxiliar, sin garantía para todas las apps.

## Ejecutar

Desde la raíz con Rust 1.92 o superior:
```
cargo run --locked -p soc
```
.env local ya preparado con `SOC_DATABASE_PATH=data/soc.sqlite3`; al copiar checkout, crear `.env` a partir de `.env.example`. La ruta es obligatoria y relativa al cwd, sin default oculto. No abre puerto ni necesita credenciales.

- Clic en la cara: dormida ↔ activa.
- Menú soc → Salir: cerrar. Menú soc → Acerca de soc: información y atribución Slint.
- Atajos M, A, Espacio y Escape, cierre con clic secundario y ayuda sobre la cara desactivados. Movimiento reducido conserva su preferencia y soporte interno para una configuración posterior.

Para una evaluación aislada: `SOC_DATABASE_PATH=/tmp/soc-review.sqlite3 cargo run --locked -p soc`. No borrar datos existentes para repetir la demostración.

## Verificar y continuar

Comandos y alcance en [testing](docs/rules/testing.md). [Arquitectura](docs/architecture/overview.md), [requisitos](docs/initialization/requirements.md), [resultado](docs/initialization/implementation.md), [decisiones](docs/decisions/README.md) y [evidencia](docs/memory/index.md). Antes de modificar, leer [AGENTS.md](AGENTS.md) y reglas afectadas. `.creation` conserva prompt e historia originales, sin sustituir la documentación vigente.

La entrega Git inicial fue autorizada posteriormente; ver [registro de entrega](docs/memory/2026-10-11-git-delivery.md). No se solicita despliegue. Los detalles gráficos son supuestos revisables; consulta la evidencia runtime antes de asumir soporte multi-monitor, Spaces o de otros OS.

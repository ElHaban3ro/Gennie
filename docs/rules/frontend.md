# Interfaz Flutter

Alcance: `frontend/lib`, assets y runners de plataforma. Leer [UI.md](../../UI.md), [modules](modules.md) y [configuration](configuration.md) antes de cambiar sus ámbitos. Flutter posee el event loop y los objetos visuales; Rust no crea ventanas. No reconstruir presencia flotante, cara ni globos.

## Límites

El frontend consume la API de `bootstrap/src/api.rs` mediante flutter_rust_bridge 2.13.0. No editar archivos generados manualmente: regenerar con la configuración raíz y revisar Rust/Dart juntos. El coordinador `bootstrap/src/runtime.rs` posee módulos y controladores, serializa operaciones y cierra recursos antes de SQLite. `apps/presence` conserva únicamente historial, preparación de respuestas y narración.

No SQL, procesos de modelos ni lógica de proveedor en widgets. Las operaciones son asíncronas y comunican errores visibles. Confirmar guardados después del commit; conservar borradores ante fallo. No hacer fallback silencioso si falta backend, modelo o configuración.

## Diseño e interacción

Mantener el nombre Gennie, icono propio, Inter 4.1 local, tamaños y navegación de [UI.md](../../UI.md). Widgets compartidos para menús y diálogos, etiquetas semánticas, teclado y foco visible. La selección de lectura no modifica el hilo activo. Estado transitorio como pestaña y ancho permanece en Dart; preferencias persistidas cruzan la API Rust.

Todos los tooltips deben usar `AppTooltip` de `frontend/lib/src/glass.dart`, también en ajustes, menús y acciones nuevas. No usar `Tooltip` de Material ni textos en la propiedad `tooltip` de los controles; para `PopupMenuButton`, dejar `tooltip: ''` y envolverlo con `AppTooltip`. Conservar un único tooltip visible, Inter 12/500, radio 8 px, padding 14 × 7, sin borde ni sombra, material nativo HUD en macOS y filtro Flutter compartido en otros destinos. Mantener semántica, presentación por foco/hover y cierre al salir, pulsar o Escape. Solo cambia la colocación según el control y el espacio disponible. `scripts/check_invariants.py` impide tooltips Material añadidos por separado; los tests de widgets comprueban el uso común.

Cerrar, minimizar y redimensionar corresponden a la ventana macOS. La integración nativa vive en el runner Flutter. No presentar transparencia dibujada como prueba de Liquid Glass nativo. Los enlaces de respuestas se abren solo por acción explícita del usuario.

## Desarrollo y verificación

`./scripts/dev.sh` prepara Rust y abre Flutter debug con hot reload; cambios Rust requieren recompilar. `--build-only` no abre UI y no compila el runner. `--generate` regenera el puente. `./scripts/build.sh` crea el paquete release con dylib, helper y licencia; no despliega.

Revisar análisis y tests Dart, tests Rust pertinentes y estado del código generado. El build macOS exige Xcode completo. Si no está disponible, informar del bloqueo: ni análisis ni widgets prueban ventana, composición, entrada física o permisos del sistema. Ver [testing](testing.md).

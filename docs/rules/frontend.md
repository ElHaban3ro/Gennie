# Interfaz Flutter

Alcance: `frontend/lib`, assets y runners de plataforma. Leer [UI.md](../../UI.md), [modules](modules.md) y [configuration](configuration.md) antes de cambiar sus ámbitos. Flutter posee el event loop y los objetos visuales; Rust no crea ventanas. No reconstruir presencia flotante, cara ni globos.

## Límites

El frontend consume la API de `bootstrap/src/api.rs` mediante flutter_rust_bridge 2.13.0. No editar archivos generados manualmente: regenerar con la configuración raíz y revisar Rust/Dart juntos. El coordinador `bootstrap/src/runtime.rs` posee módulos y controladores, serializa operaciones y cierra recursos antes de SQLite. `apps/presence` conserva únicamente historial, preparación de respuestas y narración.

No SQL, procesos de modelos ni lógica de proveedor en widgets. Las operaciones son asíncronas y comunican errores visibles. Confirmar guardados después del commit; conservar borradores ante fallo. No hacer fallback silencioso si falta backend, modelo o configuración.

## Diseño e interacción

Mantener el nombre Gennie, icono propio, Inter 4.1 local, tamaños y navegación de [UI.md](../../UI.md). Widgets compartidos para menús y diálogos, etiquetas semánticas, teclado y foco visible. La selección de lectura no modifica el hilo activo. Estado transitorio como pestaña y ancho permanece en Dart; preferencias persistidas cruzan la API Rust.

Todos los tooltips deben usar `AppTooltip` de `frontend/lib/src/glass.dart`, también en ajustes, menús y acciones nuevas. No usar `Tooltip` de Material ni textos en la propiedad `tooltip` de los controles; para `PopupMenuButton`, dejar `tooltip: ''` y envolverlo con `AppTooltip`. Conservar un único tooltip visible, radio 8 px, sin borde ni sombra; el formato compacto usa Inter 12/500 y padding 14 × 7, material nativo HUD en macOS y filtro Flutter compartido en otros destinos. Mantener semántica, presentación por foco/hover y cierre al salir, pulsar o Escape. La colocación se adapta al control y al espacio disponible. Los controles de modelos usan contenido estructurado compartido por Flutter y macOS: padding de 16 px, título Inter 13/600, estado con punto de color y etiqueta Inter 11/500, punto medio y RAM Inter 11/400. Solo ante fallo se muestra «No se pudo cargar el modelo.» en Inter 12/400, sin diagnóstico concreto; el pie Inter 11/400 solo informa del plazo de inactividad, separado por línea y 10 px. La medición de RSS se solicita únicamente al abrir el tooltip por hover; nunca desde build, actualizaciones de snapshots o temporizadores. Esta variante conserva material, radio y reglas de cierre; no simular la jerarquía mediante saltos dentro de una única etiqueta. El resto conserva el formato compacto 14 × 7. `scripts/check_invariants.py` impide tooltips Material añadidos por separado; los tests de widgets comprueban el uso común.

Cerrar, minimizar y redimensionar corresponden a la ventana macOS. La integración nativa vive en el runner Flutter. No presentar transparencia dibujada como prueba de Liquid Glass nativo. Los enlaces de respuestas se abren solo por acción explícita del usuario.

## Desarrollo y verificación

`./scripts/dev.sh` prepara Rust y abre Flutter debug con hot reload; cambios Rust requieren recompilar. `--build-only` no abre UI y no compila el runner. `--generate` regenera el puente. `./scripts/build.sh` crea el paquete release con dylib, helper y licencia; no despliega.

Revisar análisis y tests Dart, tests Rust pertinentes y estado del código generado. El build macOS exige Xcode completo. Si no está disponible, informar del bloqueo: ni análisis ni widgets prueban ventana, composición, entrada física o permisos del sistema. Ver [testing](testing.md).

## Sidebar compartida

Historial, Encargos y Plugins deben usar `AppSidebar` de `frontend/lib/src/sidebar.dart`. El ancho y el agarre pertenecen al escritorio y se conservan entre pestañas; no crear una sidebar fija independiente por módulo. Búsqueda con `SidebarSearch`, filas con `SidebarTile`, vacío lateral con `SidebarEmpty` y vacío del lector con `ContentEmpty`. Las diferencias de contenido no deben introducir otras lupas, escalas de fuente, fondos ni estructuras de búsqueda.

Los logos de Encargos son recursos de presentación del runner (`gennie/assignment-logo`), derivados de la aplicación principal guardada por Rust. Flutter no resuelve apps, hace HTTP ni consulta contenido de servicios. El runner usa iconos instalados/favicons acotados y caché regenerable; no abre sesiones. El calendario representa al encargo y el icono de la aplicación aparece en un círculo superpuesto; si no se obtiene imagen, se usa su inicial. Ver [diseño](../decisions/assignments.md#recursos-visuales-de-las-fuentes).

# Escritorio Flutter — build macOS

Verificación local del 24 de octubre de 2026. Xcode 27.1 (27A9275), Flutter 3.47.8, Dart 3.13.7 y Rust 1.95.0 en Apple Silicon.

## Compilación y paquete

`CARGO_TARGET_DIR=/tmp/gennie-flutter-target ./scripts/build.sh` terminó correctamente. Xcode compiló los plugins reales mediante Swift Package Manager y el runner; no se usó un registrador de prueba. El resultado es `frontend/build/macos/Build/Products/Release/Gennie.app`, de aproximadamente 42 MiB, con ejecutable, backend y helper arm64. Incluye Inter y su licencia.

`codesign --verify --deep --strict` pasó. La firma es local ad hoc: no implica notarización, instalación ni publicación. La biblioteca Rust release carga con `ctypes.CDLL`. Los enlaces dinámicos del backend/helper se comprobaron en el script de empaquetado.

## Puente incluido en el paquete

La prueba `frontend/test/bridge_smoke_test.dart` pasó cargando la dylib y localizando el helper dentro de esta misma `.app`, con la base temporal de `.verification/frb-smoke`. Comprobó stream de snapshots, consulta y renombrado, error de validación, conservación de mensajes, preferencias, cierre y reapertura. Los módulos de modelos, dictado, voz y Recordatorios estaban apagados en ese fixture; no consume sesión ni permisos del usuario.

## Ajuste del toolchain

La primera compilación release falló con E0463 al cargar zerofrom-derive. La firma del proc-macro era válida, pero dyld rechazó su LINKEDIT desalineado. El caso aislado falló con el perfil original y pasó con strip desactivado para herramientas de compilación. El workspace conserva `profile.release.build-override.strip=false`; ver [dependencias](../decisions/dependencies.md).

## Límites

La compilación del runner y la prueba del puente no demuestran todavía interacción física, controles de ventana, cierre/restauración desde Dock, vidrio, atajo, micrófono ni audio de la aplicación. No se abrió una instancia del usuario ni se utilizó su base de datos. Linux/Windows siguen sin validación.

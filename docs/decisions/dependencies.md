# Dependencias — revisión 2026-10-11

Elección delegada para esta construcción. Comprobado con API oficial crates.io y repositorios de mantenedores antes de incorporar. Rust local 1.95.0; mínimo workspace 1.92 por Slint. Cargo.lock se conserva para reproducibilidad, no es certificación de seguridad.

| Dependencia directa | Exacta | Features/motivo | Fuentes |
|---|---|---|---|
| Slint | 1.18.1 | defaults off; std, backend-winit, renderer-femtovg, compat-1-18, accessibility, unstable-winit-030. UI nativa, spring, políticas macOS | [crates.io](https://crates.io/crates/slint/1.18.1), [release](https://github.com/slint-ui/slint/releases/tag/v1.18.1), [MSRV/features](https://docs.rs/crate/slint/1.18.1), [spring](https://docs.slint.dev/latest/docs/slint/reference/language/animations/) |
| slint-build | 1.18.1 | build only, compila archivo UI separado | [registro](https://crates.io/crates/slint-build/1.18.1), mismo mantenedor/release |
| rusqlite | 0.40.2 | defaults off; bundled evita SQLite distinto por OS; sin extensiones/crypto | [registro](https://crates.io/crates/rusqlite/0.40.2), [mantenedor](https://github.com/rusqlite/rusqlite) |
| dotenvy | 0.15.7 | parser .env sin mutar entorno; MSRV 1.56.1 | [registro](https://crates.io/crates/dotenvy/0.15.7), [mantenedor](https://github.com/allan2/dotenvy) |
| objc2-app-kit | 0.3.2 | solo macOS; defaults off, std/NSEvent/NSView/NSWindow; cursor para hit test y política de Spaces sobre la ventana real; MSRV 1.71 | [registro](https://crates.io/crates/objc2-app-kit/0.3.2), [mantenedor](https://github.com/madsmtm/objc2) |
| sha2 | 0.11.0 | defaults off; checksum de migraciones; MSRV 1.85 | [registro](https://crates.io/crates/sha2/0.11.0), [mantenedor](https://github.com/RustCrypto/hashes) |

Winit 0.30.13 resuelto por Slint y usado mediante reexport; no segunda versión directa. API unstable se aísla y exige revisar al subir Slint. Transparencia y always-on-top son capacidades de plataforma, no garantías universales.

## Licencia
Slint permite elección Royalty-free para desktop, GPLv3 o comercial. Para esta base local se incorpora AboutSlint accesible desde el menú nativo soc → Acerca de soc. No se ha decidido licencia de distribución del proyecto ni se ha publicado un binario; la entrega Git posterior está registrada por separado. Revisar condiciones exactas al distribuir o incorporar embedded: [licencia versionada](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md), [texto royalty-free](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).

## Avisos y límites
Consultada [RustSec advisory-db](https://github.com/RustSec/advisory-db) actual: rusqlite RUSTSEC-2020-0014 y 2021-0128 están corregidos en la versión elegida; sha2 RUSTSEC-2021-0100 afecta 0.9.7, no 0.11.0. No directorio encontrado para slint/slint-build/dotenvy en ese snapshot; esto no prueba ausencia de vulnerabilidad ni compromiso.

Se extrajeron avisos correspondientes a los paquetes del lockfile completo y se compararon rangos manualmente. Las vulnerabilidades listadas están cubiertas por rangos corregidos/no afectados de las versiones resueltas, incluyendo arrayref 0.3.9 (unaffected <=0.3.9). Informativos de falta de mantenimiento: bincode 2.0.1, ttf-parser 0.25.1 y xml-rs 0.8.29. Cargo metadata filtrado para aarch64-apple-darwin confirma bincode en el grafo efectivo (typed-index-collections 3.5.0 lo introduce); ttf-parser y xml-rs no aparecen en ese grafo. Bincode se mantiene transitivo en Slint; aviso de mantenimiento, no vulnerabilidad demostrada. Reevaluar al actualizar. No equivalencia a cargo audit (no instalado), análisis de supply chain ni auditoría exhaustiva. Crates.io verifica descargas mediante checksums; revisar avisos nuevamente al actualizar.

NSEvent::mouseLocation leído en main cada 16 ms; no instala event taps ni requiere acceso a contenido de otras apps. RustSec no tenía directorio objc2-app-kit/objc2 en el snapshot consultado; no certifica ausencia de avisos. Hit test macOS dinámico: ventana ignora mouse fuera del círculo; latencia depende del event loop. Otras plataformas aún no implementan click-through.

## Extensión AppKit para Spaces — 2026-10-11
Se verificaron features NSView y NSWindow en crates.io 0.3.2 y bindings del paquete oficial (MSRV 1.71), sin nueva dependencia ni actualización de versión. Winit 0.30.13 reexporta raw-window-handle 0.6; el handle AppKit presta el NSView de la ventana viva. Acceso limitado al hilo principal, sin retener el puntero. Política con APIs AppKit documentadas: [canJoinAllSpaces](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/canjoinallspaces) y [fullScreenAuxiliary](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/fullscreenauxiliary). Avisos del paquete no se reinterpretan como certificación.

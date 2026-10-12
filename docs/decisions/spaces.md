# D-014 — La presencia acompaña al usuario entre Spaces

Fecha: 2026-10-11. Origen: fallo runtime reportado por usuario. Estado: aceptada y seguimiento entre dos escritorios normales confirmado por el usuario en instancia propia identificada. Pantalla completa y otras políticas macOS requieren evidencia específica.

Síntoma: soc permanece en el escritorio de lanzamiento. Causa identificada: desktop.rs solo configuraba nivel AlwaysOnTop y sombra; no establecía collectionBehavior para pertenecer a Spaces. El nivel flotante no cambia pertenencia. Winit 0.30.13 no ofrece esa política en WindowExtMacOS; se usa su raw handle AppKit y NSView.window() sobre la propia ventana ya creada, sin descubrir ventanas ajenas.

Arreglo mínimo dentro de desktop.rs: CanJoinAllSpaces y FullScreenAuxiliary en el NSWindow principal; conservar otros comportamientos retirando MoveToActiveSpace, FullScreenPrimary y FullScreenNone por incompatibilidad documentada. No cambiar foco, renderer, transparencia, hit test ni UI. About es ventana normal. Se habilitan NSView/NSWindow en objc2-app-kit 0.3.2 ya fijado, sin nuevas versiones/crates/capas. No se incorpora CanJoinAllApplications/Stage Manager ni seguimiento de monitor preventivo.

Verificación real, errores y límites en [memoria de la corrección](../memory/2026-10-11-spaces.md). No test que replique flags como falsa prueba del escritorio: la seam necesaria es NSWindow y cambios reales de Space. Reiniciar el binario por elección del usuario activa el arreglo; la instancia existente conserva la política anterior. Nunca detener su proceso para desplegarlo.

Fuentes oficiales: [Apple canJoinAllSpaces](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/canjoinallspaces), [Apple fullScreenAuxiliary](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/fullscreenauxiliary), [raw handles Winit](https://docs.rs/winit/0.30.13/winit/index.html).

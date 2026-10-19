# Swift Test

Sonda visual macOS 26+ independiente de SOC. Presenta una cara de 128 puntos con ojos y boca en las mismas posiciones aproximadas que la cara despierta de SOC. La barra de menús permite alternar entre `SwiftUI glassEffect`, `AppKit NSGlassEffectView` y `Fondo local (experimental)` en la misma ventana y posición. Se arrastra con el botón izquierdo; «Salir de Swift Test» la cierra.

Desde la raíz del repositorio: `sh apps/swift-test/run.sh`. Para abrir directamente la tercera variante: `sh apps/swift-test/run.sh --wallpaper`. `--origin X Y` fija temporalmente el origen AppKit de la ventana para capturas comparables.

No se registra en Cargo ni modifica la base de datos, los permisos o los procesos de SOC. La sonda no captura la pantalla. La tercera variante toma únicamente el archivo local del fondo de escritorio, lo alinea con la posición de la cara, lo desenfoca y aplica una refracción circular de borde con Core Image. Consulta solo metadatos de ventanas para volver a `NSGlassEffectView` cuando hay una app detrás. No reproduce el contenido de las ventanas ni sustituye una prueba sobre el escritorio real. La compilación no demuestra fidelidad visual; comparar capturas tomadas fuera de la app sobre el mismo fondo.

import Cocoa
import FlutterMacOS

// No se presenta ninguna ventana ni se arranca un motor o backend.
final class ReplyApplication: NSApplication {
  var replies: [Bool] = []
  override func reply(toApplicationShouldTerminate shouldTerminate: Bool) {
    replies.append(shouldTerminate)
  }
}

final class WindowEvents: NSObject, NSWindowDelegate {}

func require(_ condition: @autoclosure () -> Bool, _ message: String = "Condición de cierre incumplida") {
  if !condition() {
    FileHandle.standardError.write(Data("FALLO: \(message)\n".utf8))
    exit(1)
  }
}

@main
struct TerminationTest {
  static func main() {
    let application = ReplyApplication.shared as! ReplyApplication
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 420),
                          styleMask: [.titled], backing: .buffered, defer: true)
    let events = WindowEvents()
    window.delegate = events
    require(!window.isVisible)

    for rejected: Any? in [false, nil, "true", FlutterError(code: "fixture", message: nil, details: nil)] {
      let delegate = AppDelegate()
      var stops = 0
      delegate.stopSystemMotion = { stops += 1 }
      delegate.mainFlutterWindow = window
      delegate.completeTermination(rejected, sender: application)
      require(application.replies.last == false, "No aprobar un cierre fallido")
      require(window.delegate === events, "Conservar eventos al rechazar")
      require(stops == 0, "Conservar observador de accesibilidad al rechazar")
      require(delegate.applicationShouldTerminate(application) == .terminateNow,
                   "Un rechazo no debe dejar una solicitud pendiente")
    }

    let approved = AppDelegate()
    var stops = 0
    approved.stopSystemMotion = { stops += 1 }
    approved.mainFlutterWindow = window
    approved.completeTermination(true, sender: application)
    require(application.replies.last == true)
    require(window.delegate == nil, "Retirar eventos antes de apagar el motor")
    require(stops == 1, "Retirar observador antes de apagar el motor")
    require(approved.applicationShouldTerminate(application) == .terminateLater,
                 "No volver al canal del motor durante la terminación")
    require(application.replies.count == 5)
    require(!window.isVisible)
    print("Cierre nativo: aprobación, cuatro rechazos y duplicado correctos; sin UI/modelos/DB.")
  }
}

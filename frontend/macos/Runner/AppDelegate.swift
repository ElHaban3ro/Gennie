import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var lifecycle: FlutterMethodChannel?
  private var terminationPending = false
  var stopSystemMotion: (() -> Void)?

  func configureLifecycle(_ controller: FlutterViewController) {
    lifecycle = FlutterMethodChannel(name: "gennie/lifecycle", binaryMessenger: controller.engine.binaryMessenger)
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  override func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
    guard !terminationPending else { return .terminateLater }
    guard let lifecycle else { return .terminateNow }
    terminationPending = true
    lifecycle.invokeMethod("requestQuit", arguments: nil) { [weak self] result in
      guard let self else {
        sender.reply(toApplicationShouldTerminate: false)
        return
      }
      self.completeTermination(result, sender: sender)
    }
    return .terminateLater
  }

  func completeTermination(_ result: Any?, sender: NSApplication) {
    let approved = (result as? Bool) == true
    terminationPending = approved
    if approved {
      // AppKit cierra las ventanas después de apagar Flutter. Retirar sus eventos
      // solo tras confirmar el cierre del backend evita envíos al motor destruido.
      mainFlutterWindow?.delegate = nil
      stopSystemMotion?()
    }
    sender.reply(toApplicationShouldTerminate: approved)
  }

  override func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
    if !flag, let window = mainFlutterWindow {
      window.makeKeyAndOrderFront(nil)
      sender.activate(ignoringOtherApps: true)
    }
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}

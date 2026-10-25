import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var lifecycle: FlutterMethodChannel?
  private var terminationPending = false

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
      self?.terminationPending = false
      sender.reply(toApplicationShouldTerminate: (result as? Bool) == true)
    }
    return .terminateLater
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

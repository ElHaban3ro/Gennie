import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let controller = FlutterViewController()
    let windowFrame = NSRect(origin: frame.origin, size: NSSize(width: 1100, height: 720))
    contentViewController = controller
    setFrame(windowFrame, display: true)
    contentMinSize = NSSize(width: 640, height: 420)
    title = "Gennie"
    titleVisibility = .hidden
    titlebarAppearsTransparent = true
    styleMask.insert(.fullSizeContentView)
    isOpaque = false
    backgroundColor = .clear
    controller.backgroundColor = .clear
    hasShadow = false
    if let content = contentView {
      let material: NSView
      if #available(macOS 26.0, *) {
        let glass = NSGlassEffectView(frame: content.bounds)
        glass.style = .regular
        material = glass
      } else {
        let effect = NSVisualEffectView(frame: content.bounds)
        effect.material = .underWindowBackground
        effect.blendingMode = .behindWindow
        effect.state = .active
        material = effect
      }
      material.autoresizingMask = [.width, .height]
      content.addSubview(material, positioned: .below, relativeTo: nil)
    }
    (NSApp.delegate as? AppDelegate)?.configureLifecycle(controller)
    RegisterGeneratedPlugins(registry: controller)
    super.awakeFromNib()
    positionWindowButtons()
  }
  override func layoutIfNeeded() {
    super.layoutIfNeeded()
    positionWindowButtons()
  }
  private func positionWindowButtons() {
    for (index, kind) in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton].enumerated() {
      if let button = standardWindowButton(kind) {
        var frame = button.frame
        frame.origin.x = 72 + CGFloat(index) * 22
        button.setFrameOrigin(frame.origin)
      }
    }
  }
}
